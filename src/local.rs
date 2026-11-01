use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tempfile::TempDir;

use crate::assert::{Snapshot, SnapshotAsset, SnapshotRelease};
use crate::process::{
    base_env, copy_dir_all, expand_artifact, git, parse_outputs, run_cr, run_shell, write_file,
};
use crate::scenarios::{rerun_input, SandboxConfig, Scenario, Trigger};
use fake_github::FakeGitHub;

#[derive(Clone, Debug)]
pub struct LocalOptions {
    pub keep: bool,
    pub cr_bin: String,
    pub template: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct LocalRunSet {
    pub initial: Snapshot,
    pub rerun: Option<Snapshot>,
}

struct LocalRepo {
    repo: TempDir,
    remote: TempDir,
    dir: PathBuf,
}

impl LocalRepo {
    fn new(template: &Path) -> Result<Self> {
        let repo = tempfile::Builder::new()
            .prefix("cr-local-")
            .tempdir()
            .context("creating the local repository")?;
        let remote = tempfile::Builder::new()
            .prefix("cr-local-remote-")
            .tempdir()
            .context("creating the local remote")?;
        let dir = repo.path().to_path_buf();
        let remote_dir = remote.path().to_path_buf();

        copy_dir_all(template, &dir)?;
        git(&dir, ["init", "-q", "-b", "main"])?;
        git(
            &dir,
            ["config", "user.email", "harness@continuous-release.invalid"],
        )?;
        git(&dir, ["config", "user.name", "continuous-release harness"])?;
        git(&dir, ["add", "-A"])?;
        git(&dir, ["commit", "-q", "-m", "sandbox: base"])?;
        git(&dir, ["tag", "base"])?;

        git(&remote_dir, ["init", "--bare", "-q"])?;
        git(
            &dir,
            [
                "remote",
                "add",
                "origin",
                remote_dir.to_string_lossy().as_ref(),
            ],
        )?;
        git(&dir, ["push", "-q", "origin", "main"])?;
        git(&dir, ["push", "-q", "origin", "refs/tags/base"])?;

        Ok(Self { repo, remote, dir })
    }

    fn apply(&self, scenario: &Scenario, config: &SandboxConfig, fake: &FakeGitHub) -> Result<()> {
        for tag in &scenario.initial_tags {
            git(&self.dir, ["tag", "-f", tag])?;
            let tag_ref = format!("refs/tags/{tag}");
            git(
                &self.dir,
                ["push", "-q", "--force", "origin", tag_ref.as_str()],
            )?;
            fake.seed_tag(tag, &self.head()?);
        }
        for tag in &scenario.external_tags {
            let sha = self.create_orphan_tag(tag)?;
            fake.seed_tag(tag, &sha);
        }

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
            self.commit(commit.message, commit.body, &files)?;
        }

        if scenario.commits.is_empty() && scenario.config.is_some() {
            write_file(
                &self.dir.join("scenario.json"),
                &format!("{}\n", serde_json::to_string_pretty(config)?),
            )?;
            self.commit("chore: configure the harness scenario", None, &[])?;
        }
        Ok(())
    }

    fn commit(
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
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&changelog)
                .with_context(|| format!("opening {}", changelog.display()))?;
            writeln!(file, "\n- {message}")?;
        }

        git(&self.dir, ["add", "-A"])?;
        let mut args: Vec<&str> = vec!["commit", "-q", "-m", message];
        if let Some(body) = body {
            args.push("-m");
            args.push(body);
        }
        git(&self.dir, args)?;
        self.head()
    }

    fn create_orphan_tag(&self, name: &str) -> Result<String> {
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
            ["commit", "-q", "--allow-empty", "-m", message.as_str()],
        )?;
        let sha = self.head()?;
        git(&self.dir, ["tag", "-f", name])?;
        let tag_ref = format!("refs/tags/{name}");
        git(
            &self.dir,
            ["push", "-q", "--force", "origin", tag_ref.as_str()],
        )?;
        git(&self.dir, ["checkout", "-f", "main"])?;
        git(&self.dir, ["branch", "-D", &branch])?;
        Ok(sha)
    }

    fn sync_tag(&self, name: &str, sha: &str) -> Result<()> {
        git(&self.dir, ["tag", "-f", name, sha])?;
        let tag_ref = format!("refs/tags/{name}");
        git(
            &self.dir,
            ["push", "-q", "--force", "origin", tag_ref.as_str()],
        )?;
        Ok(())
    }

    fn remove_tag(&self, name: &str) {
        let _ = git(&self.dir, ["tag", "-d", name]);
        let delete_ref = format!(":refs/tags/{name}");
        let _ = git(&self.dir, ["push", "-q", "origin", delete_ref.as_str()]);
    }

    fn head(&self) -> Result<String> {
        git(&self.dir, ["rev-parse", "HEAD"])
    }

    fn persist(self) -> Result<()> {
        let repo = self.repo.keep();
        let remote = self.remote.keep();
        println!("  kept {} and {}", repo.display(), remote.display());
        Ok(())
    }
}

fn failure_snapshot(fake: &FakeGitHub, error: impl std::fmt::Display) -> Snapshot {
    Snapshot {
        conclusion: Some("failure".into()),
        releases: fake
            .list_releases()
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
            .collect(),
        tags: fake.list_tags(),
        error: Some(format!("{error:#}")),
    }
}

fn success_snapshot(fake: &FakeGitHub) -> Snapshot {
    Snapshot {
        conclusion: Some("success".into()),
        releases: fake
            .list_releases()
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
            .collect(),
        tags: fake.list_tags(),
        error: None,
    }
}

fn cr_args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// Run the same sequence as auto-release.yml against a local repository and a
/// fake GitHub. The shared workflow itself is only exercised live; this covers
/// the contract, the toolkit, and the build commands in seconds.
fn run_contract(
    repo: &LocalRepo,
    fake: &FakeGitHub,
    config: &SandboxConfig,
    cr_bin: &str,
    publish_override: Option<&str>,
) -> Result<Snapshot> {
    let sha = repo.head()?;
    let working = repo.dir.join(&config.directory);
    if !working.is_dir() {
        return Ok(failure_snapshot(
            fake,
            format!("fixture directory {} does not exist", config.directory),
        ));
    }

    let output_path = repo.dir.join(".github-output");
    write_file(&output_path, "")?;
    let notes_path = repo.dir.join("release-notes.md");
    let mut env = base_env();
    env.insert("GITHUB_REPOSITORY".into(), "local/repo".into());
    env.insert("GITHUB_TOKEN".into(), "local".into());
    env.insert("GITHUB_SHA".into(), sha.clone());
    env.insert(
        "GITHUB_OUTPUT".into(),
        output_path.to_string_lossy().to_string(),
    );

    let mut reserve_args = cr_args(&[
        "reserve",
        "--repo",
        "local/repo",
        "--api-url",
        fake.url(),
        "--token",
        "local",
        "--target",
        &sha,
        "--notes-out",
        notes_path.to_string_lossy().as_ref(),
    ]);
    reserve_args.retain(|arg| !arg.is_empty());

    let outputs = match run_cr(cr_bin, &reserve_args, &working, &env) {
        Ok(_) => parse_outputs(&output_path)?,
        Err(error) => return Ok(failure_snapshot(fake, error)),
    };

    if outputs.get("superseded").map(String::as_str) == Some("true") {
        return Ok(success_snapshot(fake));
    }

    let version = outputs.get("version").cloned().unwrap_or_default();
    let tag = outputs.get("tag").cloned().unwrap_or_default();
    if !tag.is_empty() {
        repo.sync_tag(&tag, &sha)?;
    }

    let mut versioned_env = env.clone();
    versioned_env.insert("VERSION".into(), version);

    let mut build_ok = false;
    let attempt = (|| -> Result<()> {
        run_shell(&config.setup, &working, &versioned_env)?;
        run_shell(&config.test, &working, &versioned_env)?;
        run_shell(&config.build, &working, &versioned_env)?;
        build_ok = true;

        let publish = publish_override.unwrap_or(config.publish.as_str());
        run_shell(publish, &working, &versioned_env)?;

        let assets = expand_artifact(&config.artifact, &working)?;
        if !config.artifact.trim().is_empty() && assets.is_empty() {
            bail!("no artifact matched {}", config.artifact);
        }

        let mut release_args = cr_args(&[
            "release",
            "--repo",
            "local/repo",
            "--api-url",
            fake.url(),
            "--token",
            "local",
            "--tag",
            &tag,
            "--target",
            &sha,
            "--notes-file",
            notes_path.to_string_lossy().as_ref(),
        ]);
        for asset in &assets {
            release_args.push("--asset".into());
            release_args.push(asset.to_string_lossy().to_string());
        }
        run_cr(cr_bin, &release_args, &working, &versioned_env)?;
        Ok(())
    })();

    match attempt {
        Ok(()) => Ok(success_snapshot(fake)),
        Err(error) => {
            if !build_ok && !tag.is_empty() {
                let unreserve_args = cr_args(&[
                    "unreserve",
                    "--repo",
                    "local/repo",
                    "--api-url",
                    fake.url(),
                    "--token",
                    "local",
                    "--tag",
                    &tag,
                    "--target",
                    &sha,
                ]);
                if run_cr(cr_bin, &unreserve_args, &working, &env).is_ok() {
                    repo.remove_tag(&tag);
                }
            }
            Ok(failure_snapshot(fake, error))
        }
    }
}

pub fn run_local_scenario(scenario: &Scenario, options: &LocalOptions) -> Result<LocalRunSet> {
    let config = SandboxConfig::default().merged(&scenario.config);
    let repo = LocalRepo::new(&options.template)?;
    let mut fake = FakeGitHub::new();
    fake.start()?;

    let result = (|| -> Result<LocalRunSet> {
        repo.apply(scenario, &config, &fake)?;
        let working = repo.dir.join(&config.directory);

        if scenario.trigger == Trigger::PullRequest {
            let mut env = base_env();
            env.insert("GITHUB_REPOSITORY".into(), "local/repo".into());
            env.insert("GITHUB_TOKEN".into(), "local".into());
            env.insert("GITHUB_SHA".into(), repo.head()?);

            let attempt = (|| -> Result<()> {
                run_shell(&config.setup, &working, &env)?;
                run_shell(&config.test, &working, &env)?;
                Ok(())
            })();
            let initial = match attempt {
                Ok(()) => success_snapshot(&fake),
                Err(error) => failure_snapshot(&fake, error),
            };
            return Ok(LocalRunSet {
                initial,
                rerun: None,
            });
        }

        let initial = run_contract(&repo, &fake, &config, &options.cr_bin, None)?;
        if let Some(error) = &initial.error {
            println!("  local: {error}");
        }

        let mut rerun = None;
        if scenario.rerun {
            println!("  re-running the same commit locally");
            let snapshot = run_contract(
                &repo,
                &fake,
                &config,
                &options.cr_bin,
                rerun_input(scenario, "publish-override"),
            )?;
            if let Some(error) = &snapshot.error {
                println!("  local re-run: {error}");
            }
            rerun = Some(snapshot);
        }

        Ok(LocalRunSet { initial, rerun })
    })();

    fake.stop();
    if options.keep {
        repo.persist()?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_snapshot_keeps_release_state() {
        let fake = FakeGitHub::new();
        fake.seed_tag("v1.2.3", "abc");
        let snapshot = failure_snapshot(&fake, "boom");
        assert_eq!(snapshot.conclusion.as_deref(), Some("failure"));
        assert_eq!(snapshot.tags, vec!["v1.2.3"]);
    }
}
