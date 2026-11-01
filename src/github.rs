use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use anyhow::{anyhow, Result};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::blocking::Client;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "GitHub API {}: {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReleaseAsset {
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Release {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TagRef {
    #[serde(default, rename = "ref")]
    pub reference: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WorkflowRun {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub head_sha: String,
    #[serde(default)]
    pub head_branch: Option<String>,
    #[serde(default)]
    pub html_url: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PullRequest {
    #[serde(default)]
    pub number: u64,
    #[serde(default)]
    pub html_url: String,
}

#[derive(Clone, Debug)]
pub struct GitHub {
    token: String,
    repository: String,
    api_url: String,
    client: Client,
}

impl GitHub {
    pub fn new(token: impl Into<String>, repository: impl Into<String>) -> Result<Self> {
        Self::with_api_url(token, repository, None)
    }

    pub fn with_api_url(
        token: impl Into<String>,
        repository: impl Into<String>,
        api_url: Option<String>,
    ) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;
        Ok(Self {
            token: token.into(),
            repository: repository.into(),
            api_url: api_url
                .unwrap_or_else(|| "https://api.github.com".into())
                .trim_end_matches('/')
                .to_string(),
            client,
        })
    }

    fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_url, path);
        let mut request = self
            .client
            .request(method.clone(), &url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "continuous-release-harness");
        if let Some(body) = &body {
            request = request.json(body);
        }
        let response = request
            .send()
            .map_err(|error| anyhow!("GitHub {} {path}: {error}", method.as_str()))?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().unwrap_or_default();
            return Err(ApiError {
                status: status.as_u16(),
                message,
            }
            .into());
        }
        if status.as_u16() == 204 {
            return Ok(serde_json::from_value(Value::Null)?);
        }
        let text = response.text()?;
        if text.trim().is_empty() {
            return Ok(serde_json::from_value(Value::Null)?);
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub fn list_releases(&self) -> Result<Vec<Release>> {
        self.request(
            Method::GET,
            &format!("/repos/{}/releases?per_page=100", self.repository),
            None,
        )
    }

    pub fn delete_release(&self, id: u64) -> Result<()> {
        self.request::<()>(
            Method::DELETE,
            &format!("/repos/{}/releases/{}", self.repository, id),
            None,
        )
    }

    pub fn list_tag_refs(&self) -> Result<Vec<TagRef>> {
        self.request(
            Method::GET,
            &format!("/repos/{}/git/refs/tags?per_page=100", self.repository),
            None,
        )
    }

    pub fn delete_tag(&self, tag: &str) -> Result<()> {
        self.request::<()>(
            Method::DELETE,
            &format!("/repos/{}/git/refs/tags/{}", self.repository, encode(tag)),
            None,
        )
    }

    pub fn delete_all_releases(&self) -> Result<()> {
        for release in self.list_releases()? {
            self.delete_release(release.id)?;
        }
        Ok(())
    }

    pub fn delete_all_version_tags(&self) -> Result<()> {
        for reference in self.list_tag_refs()? {
            let tag = reference
                .reference
                .strip_prefix("refs/tags/")
                .unwrap_or(&reference.reference);
            if tag.starts_with('v') {
                self.delete_tag(tag)?;
            }
        }
        Ok(())
    }

    pub fn list_runs(&self, workflow: &str, head_sha: Option<&str>) -> Result<Vec<WorkflowRun>> {
        let mut path = format!(
            "/repos/{}/actions/workflows/{}/runs?per_page=30",
            self.repository, workflow
        );
        if let Some(head_sha) = head_sha {
            path.push_str("&head_sha=");
            path.push_str(&encode(head_sha));
        }
        let data: Value = self.request(Method::GET, &path, None)?;
        let runs = data
            .get("workflow_runs")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        Ok(serde_json::from_value(runs)?)
    }

    pub fn dispatch_workflow(
        &self,
        workflow: &str,
        reference: &str,
        inputs: &HashMap<String, String>,
    ) -> Result<()> {
        let inputs: HashMap<&String, &String> = inputs.iter().collect();
        self.request::<()>(
            Method::POST,
            &format!(
                "/repos/{}/actions/workflows/{}/dispatches",
                self.repository, workflow
            ),
            Some(json!({ "ref": reference, "inputs": inputs })),
        )
    }

    pub fn create_pull_request(
        &self,
        title: &str,
        head: &str,
        base: &str,
        body: &str,
    ) -> Result<PullRequest> {
        self.request(
            Method::POST,
            &format!("/repos/{}/pulls", self.repository),
            Some(json!({
                "title": title,
                "head": head,
                "base": base,
                "body": body,
            })),
        )
    }

    pub fn close_pull_request(&self, number: u64) -> Result<()> {
        self.request::<()>(
            Method::PATCH,
            &format!("/repos/{}/pulls/{}", self.repository, number),
            Some(json!({ "state": "closed" })),
        )
    }

    pub fn delete_branch(&self, branch: &str) -> Result<()> {
        match self.request::<()>(
            Method::DELETE,
            &format!(
                "/repos/{}/git/refs/heads/{}",
                self.repository,
                encode(branch)
            ),
            None,
        ) {
            Ok(()) => Ok(()),
            Err(error) => {
                if error
                    .downcast_ref::<ApiError>()
                    .map(|api| api.status == 422)
                    .unwrap_or(false)
                {
                    Ok(())
                } else {
                    Err(error)
                }
            }
        }
    }
}

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}
