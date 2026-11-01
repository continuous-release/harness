use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tempfile::TempDir;

use crate::process::{git, git_inherit, write_file};
use crate::scenarios::SandboxConfig;

pub struct Sandbox {
    _repo: TempDir,
    dir: PathBuf,
    remote: String,
}

impl Sandbox {
    pub fn new(repository: &str, token: &str) -> Result<Self> {
        let repo = tempfile::Builder::new()
            .prefix("cr-sandbox-")
            .tempdir()
            .context("creating the sandbox directory")?;
        let dir = repo.path().to_path_buf();
        let remote = format!("https://x-access-token:{token}@github.com/{repository}.git");
        Ok(Self {
            _repo: repo,
            dir,
            remote,
        })
    }

    fn ensure_clone(&self) -> Result<()> {
        if self.dir.join(".git").exists() {
            return Ok(());
        }
        let args: Vec<OsString> = vec![
            "clone".into(),
            self.remote.clone().into(),
            self.dir.clone().into(),
        ];
        git_inherit(Path::new("."), args)?;
        git(
            &self.dir,
            ["config", "user.email", "harness@continuous-release.invalid"],
        )?;
        git(
            &self.dir,
            ["config", "user.name", "continuous-release harness"],
        )?;
        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        self.ensure_clone()?;
        git(
            &self.dir,
            [
                "fetch",
                "origin",
                "refs/tags/base:refs/tags/base",
                "--force",
            ],
        )?;
        git(&self.dir, ["checkout", "-B", "main", "base"])?;
        git(&self.dir, ["reset", "--hard", "base"])?;
        git(&self.dir, ["clean", "-fdx"])?;
        let tags = git(&self.dir, ["tag", "-l", "v*"])?;
        for tag in tags.lines().filter(|tag| !tag.is_empty()) {
            git(&self.dir, ["tag", "-d", tag])?;
        }
        Ok(())
    }

    pub fn write_scenario(&self, config: &SandboxConfig) -> Result<()> {
        write_file(
            &self.dir.join("scenario.json"),
            &format!("{}\n", serde_json::to_string_pretty(config)?),
        )
    }

    pub fn commit(
        &self,
        message: &str,
        body: Option<&str>,
        files: &[(String, String)],
    ) -> Result<String> {
        for (path, content) in files {
            write_file(&self.dir.join(path), content)?;
        }
        if !files.iter().any(|(path, _)| path == "CHANGELOG.md") {
            let changelog = self.dir.join("CHANGELOG.md");
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&changelog)
                .with_context(|| format!("opening {}", changelog.display()))?;
            use std::io::Write;
            writeln!(file, "\n- {message}")?;
        }

        git(&self.dir, ["add", "-A"])?;
        let mut args: Vec<&str> = vec!["commit", "-m", message];
        if let Some(body) = body {
            args.push("-m");
            args.push(body);
        }
        git(&self.dir, args)?;
        self.head()
    }

    pub fn tag(&self, name: &str) -> Result<()> {
        git(&self.dir, ["tag", "-f", name])?;
        Ok(())
    }

    /// Create a tag on an unrelated commit, as a concurrent release would. The
    /// commit is not an ancestor of main, so version derivation still sees the
    /// previous tag but tag reservation collides.
    pub fn create_orphan_tag(&self, name: &str) -> Result<()> {
        let safe: String = name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect();
        let branch = format!("orphan-{safe}");
        git(&self.dir, ["checkout", "--orphan", &branch])?;
        git(&self.dir, ["rm", "-rf", "--cached", "."])?;
        let message = format!("chore: orphan commit for {name}");
        git(
            &self.dir,
            ["commit", "--allow-empty", "-m", message.as_str()],
        )?;
        git(&self.dir, ["tag", "-f", name])?;
        let tag_ref = format!("refs/tags/{name}");
        git(&self.dir, ["push", "--force", "origin", tag_ref.as_str()])?;
        git(&self.dir, ["checkout", "-f", "main"])?;
        git(&self.dir, ["branch", "-D", &branch])?;
        Ok(())
    }

    pub fn push_tag(&self, name: &str) -> Result<()> {
        let tag_ref = format!("refs/tags/{name}");
        git(&self.dir, ["push", "--force", "origin", tag_ref.as_str()])?;
        Ok(())
    }

    pub fn push_main(&self) -> Result<()> {
        git(&self.dir, ["push", "--force", "origin", "main"])?;
        Ok(())
    }

    pub fn push_branch(&self, name: &str) -> Result<()> {
        let refspec = format!("{name}:{name}");
        git(&self.dir, ["push", "--force", "origin", refspec.as_str()])?;
        Ok(())
    }

    pub fn checkout_new_branch(&self, name: &str) -> Result<()> {
        git(&self.dir, ["checkout", "-B", name, "base"])?;
        Ok(())
    }

    pub fn delete_local_branch(&self, name: &str) {
        let _ = git(&self.dir, ["branch", "-D", name]);
    }

    pub fn head(&self) -> Result<String> {
        git(&self.dir, ["rev-parse", "HEAD"])
    }
}
