use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use crate::assert::{assert_run_set, assert_snapshot, Snapshot, SnapshotAsset, SnapshotRelease};
use crate::github::{GitHub, WorkflowRun};
use crate::sandbox::Sandbox;
use crate::scenarios::{SandboxConfig, Scenario, Trigger};

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub keep: bool,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct ScenarioResult {
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
    pub run_url: Option<String>,
}

fn wait_for_run<F>(
    github: &GitHub,
    workflow: &str,
    predicate: F,
    timeout: Duration,
) -> Result<WorkflowRun>
where
    F: Fn(&WorkflowRun) -> bool,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let runs = github.list_runs(workflow, None)?;
        if let Some(run) = runs.iter().find(|run| predicate(run)) {
            if run.status == "completed" {
                return Ok(run.clone());
            }
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    bail!(
        "Timed out waiting for {workflow} after {}s",
        timeout.as_secs()
    )
}

fn dispatch_and_wait(
    github: &GitHub,
    workflow: &str,
    reference: &str,
    timeout: Duration,
    inputs: &HashMap<String, String>,
) -> Result<WorkflowRun> {
    let before = github.list_runs(workflow, None)?;
    let max_id = before.iter().map(|run| run.id).max().unwrap_or(0);
    github.dispatch_workflow(workflow, reference, inputs)?;
    wait_for_run(
        github,
        workflow,
        |run| run.id > max_id && run.event == "workflow_dispatch",
        timeout,
    )
}

fn snapshot_of(github: &GitHub, run: &WorkflowRun) -> Result<Snapshot> {
    let releases = github
        .list_releases()?
        .into_iter()
        .map(|release| SnapshotRelease {
            tag_name: release.tag_name,
            prerelease: release.prerelease,
            body: release.body,
            assets: release
                .assets
                .into_iter()
                .map(|asset| SnapshotAsset { name: asset.name })
                .collect(),
        })
        .collect();
    let tags = github
        .list_tag_refs()?
        .into_iter()
        .map(|reference| {
            reference
                .reference
                .strip_prefix("refs/tags/")
                .unwrap_or(&reference.reference)
                .to_string()
        })
        .collect();
    Ok(Snapshot {
        conclusion: run.conclusion.clone(),
        releases,
        tags,
        error: None,
    })
}

fn scenario_commits(
    sandbox: &Sandbox,
    scenario: &Scenario,
    config: &SandboxConfig,
) -> Result<String> {
    for (index, commit) in scenario.commits.iter().enumerate() {
        let mut files: Vec<(String, String)> = commit
            .files
            .iter()
            .map(|(path, content)| (path.to_string(), content.to_string()))
            .collect();
        if index == 0 {
            files.push((
                "scenario.json".to_string(),
                format!("{}\n", serde_json::to_string_pretty(config)?),
            ));
        }
        sandbox.commit(commit.message, commit.body, &files)?;
    }
    if scenario.commits.is_empty() && scenario.config.is_some() {
        sandbox.write_scenario(config)?;
        sandbox.commit("chore: configure the harness scenario", None, &[])?;
    }
    sandbox.head()
}

fn run_pull_request_scenario(
    scenario: &Scenario,
    github: &GitHub,
    sandbox: &Sandbox,
    options: &RunOptions,
) -> Result<WorkflowRun> {
    let branch = format!("scenario/{}", scenario.name);
    github.delete_branch(&branch)?;

    let mut pr_number: Option<u64> = None;
    let result = (|| -> Result<WorkflowRun> {
        sandbox.checkout_new_branch(&branch)?;
        let config = SandboxConfig::default().merged(&scenario.config);
        let sha = scenario_commits(sandbox, scenario, &config)?;
        sandbox.push_branch(&branch)?;

        let pull_request = github.create_pull_request(
            &format!("harness: {}", scenario.name),
            &branch,
            "main",
            scenario.description,
        )?;
        pr_number = Some(pull_request.number);
        println!("opened {}", pull_request.html_url);

        let run = wait_for_run(
            github,
            "ci.yml",
            |candidate| {
                candidate.event == "pull_request"
                    && candidate.head_branch.as_deref() == Some(branch.as_str())
            },
            options.timeout,
        )?;
        if run.conclusion.as_deref() != Some("success") {
            bail!("pull request CI failed: {}", run.html_url);
        }

        let release_runs = github.list_runs("release.yml", Some(&sha))?;
        if !release_runs.is_empty() {
            let urls: Vec<&str> = release_runs
                .iter()
                .map(|candidate| candidate.html_url.as_str())
                .collect();
            bail!(
                "release workflow ran for a pull request: {}",
                urls.join(", ")
            );
        }

        let snapshot = snapshot_of(github, &run)?;
        assert_snapshot(&scenario.expect, &scenario.initial_tags, &snapshot)?;
        Ok(run)
    })();

    if let Some(number) = pr_number {
        let _ = github.close_pull_request(number);
    }
    let _ = github.delete_branch(&branch);
    let reset = sandbox.reset();
    sandbox.delete_local_branch(&branch);
    match result {
        Ok(run) => {
            reset?;
            Ok(run)
        }
        Err(error) => {
            let _ = reset;
            Err(error)
        }
    }
}

pub fn run_scenario(
    scenario: &Scenario,
    github: &GitHub,
    sandbox: &Sandbox,
    options: &RunOptions,
) -> Result<WorkflowRun> {
    println!("\n=== {} ===", scenario.name);
    println!("{}", scenario.description);

    github.delete_all_releases()?;
    github.delete_all_version_tags()?;
    sandbox.reset()?;

    if scenario.trigger == Trigger::PullRequest {
        return run_pull_request_scenario(scenario, github, sandbox, options);
    }

    let config = SandboxConfig::default().merged(&scenario.config);
    for tag in &scenario.initial_tags {
        sandbox.tag(tag)?;
        sandbox.push_tag(tag)?;
    }
    for tag in &scenario.external_tags {
        sandbox.create_orphan_tag(tag)?;
    }

    let sha = scenario_commits(sandbox, scenario, &config)?;
    sandbox.push_main()?;

    let run = if scenario.trigger == Trigger::Dispatch {
        dispatch_and_wait(
            github,
            "release.yml",
            "main",
            options.timeout,
            &HashMap::new(),
        )?
    } else {
        wait_for_run(
            github,
            "release.yml",
            |candidate| candidate.event == "push" && candidate.head_sha == sha,
            options.timeout,
        )?
    };

    println!(
        "workflow: {} ({})",
        run.html_url,
        run.conclusion.as_deref().unwrap_or("none")
    );
    let initial = snapshot_of(github, &run)?;

    let mut rerun_snapshot = None;
    if scenario.rerun {
        println!("dispatching the same commit again");
        let mut inputs = HashMap::new();
        for (key, value) in &scenario.rerun_inputs {
            inputs.insert(key.to_string(), value.to_string());
        }
        let rerun = dispatch_and_wait(github, "release.yml", "main", options.timeout, &inputs)?;
        println!(
            "rerun: {} ({})",
            rerun.html_url,
            rerun.conclusion.as_deref().unwrap_or("none")
        );
        rerun_snapshot = Some(snapshot_of(github, &rerun)?);
    }

    assert_run_set(scenario, &initial, rerun_snapshot.as_ref())?;

    if !options.keep {
        github.delete_all_releases()?;
        github.delete_all_version_tags()?;
        sandbox.reset()?;
        sandbox.push_main()?;
    }

    Ok(run)
}
