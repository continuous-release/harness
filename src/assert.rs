use anyhow::{bail, Result};

use crate::scenarios::{Expectation, Scenario, WorkflowExpectation};

#[derive(Clone, Debug, Default)]
pub struct SnapshotAsset {
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct SnapshotRelease {
    pub tag_name: String,
    pub prerelease: bool,
    pub body: Option<String>,
    pub assets: Vec<SnapshotAsset>,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub conclusion: Option<String>,
    pub releases: Vec<SnapshotRelease>,
    pub tags: Vec<String>,
    pub error: Option<String>,
}

pub fn version_tags(tags: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = tags
        .iter()
        .filter(|tag| tag.starts_with('v'))
        .cloned()
        .collect();
    tags.sort();
    tags
}

fn matches(pattern: &str, name: &str) -> Result<bool> {
    Ok(glob::Pattern::new(pattern)?.matches(name))
}

/// Assert one run against the same expectations in both modes. Local runs
/// build the snapshot from the fake GitHub; live runs build it from the API.
pub fn assert_snapshot(
    expected: &Expectation,
    initial_tags: &[&str],
    snapshot: &Snapshot,
) -> Result<()> {
    let expect_failure = expected.workflow == Some(WorkflowExpectation::Failure);
    if expect_failure {
        if snapshot.conclusion.as_deref() != Some("failure") {
            bail!(
                "expected a failed workflow, got {}",
                snapshot.conclusion.as_deref().unwrap_or("none")
            );
        }
    } else if snapshot.conclusion.as_deref() != Some("success") {
        bail!(
            "workflow failed: {}",
            snapshot.conclusion.as_deref().unwrap_or("none")
        );
    }

    if expected.release == Some(false) {
        if !snapshot.releases.is_empty() {
            let names: Vec<&str> = snapshot
                .releases
                .iter()
                .map(|release| release.tag_name.as_str())
                .collect();
            bail!("expected no releases, got {}", names.join(", "));
        }
        let tags = version_tags(&snapshot.tags);
        let mut initial: Vec<String> = expected
            .tags
            .as_ref()
            .map(|tags| tags.iter().map(|tag| tag.to_string()).collect())
            .unwrap_or_else(|| initial_tags.iter().map(|tag| tag.to_string()).collect());
        initial.sort();
        if tags != initial {
            bail!("expected tags {:?}, got {:?}", initial, tags);
        }
        return Ok(());
    }

    let tag = expected.tag.ok_or_else(|| {
        anyhow::anyhow!("scenario must set expect.tag when a release is expected")
    })?;
    let release = snapshot
        .releases
        .iter()
        .find(|candidate| candidate.tag_name == tag)
        .ok_or_else(|| anyhow::anyhow!("release {tag} was not created"))?;

    if let Some(prerelease) = expected.prerelease {
        if release.prerelease != prerelease {
            bail!(
                "expected prerelease={prerelease}, got {}",
                release.prerelease
            );
        }
    }

    for pattern in &expected.assets {
        if !release
            .assets
            .iter()
            .any(|asset| matches(pattern, &asset.name).unwrap_or(false))
        {
            let names: Vec<&str> = release
                .assets
                .iter()
                .map(|asset| asset.name.as_str())
                .collect();
            bail!("no asset matched {pattern}; assets: {}", names.join(", "));
        }
    }

    for text in &expected.notes_include {
        if !release
            .body
            .as_deref()
            .map(|body| body.contains(text))
            .unwrap_or(false)
        {
            bail!("notes did not include {:?}", text);
        }
    }

    for text in &expected.notes_exclude {
        if release
            .body
            .as_deref()
            .map(|body| body.contains(text))
            .unwrap_or(false)
        {
            bail!("notes unexpectedly included {:?}", text);
        }
    }

    Ok(())
}

/// Assert the initial run and, when the scenario asks for it, the re-run. The
/// re-run must keep the same tag, create exactly one asset, and not add a tag.
pub fn assert_run_set(
    scenario: &Scenario,
    initial: &Snapshot,
    rerun: Option<&Snapshot>,
) -> Result<()> {
    assert_snapshot(&scenario.expect, &scenario.initial_tags, initial)?;

    if !scenario.rerun {
        return Ok(());
    }

    let rerun = rerun.ok_or_else(|| anyhow::anyhow!("scenario expected a re-run snapshot"))?;
    let expected = scenario.rerun_expect.as_ref().unwrap_or(&scenario.expect);
    assert_snapshot(expected, &scenario.initial_tags, rerun)?;

    let tags = version_tags(&rerun.tags);
    let expected_count = scenario.initial_tags.len() + 1;
    if tags.len() != expected_count {
        bail!(
            "expected {expected_count} version tags after a re-run, got {:?}",
            tags
        );
    }

    let tag = expected.tag.ok_or_else(|| {
        anyhow::anyhow!("scenario must set expect.tag when a release is expected")
    })?;
    let release = rerun
        .releases
        .iter()
        .find(|candidate| candidate.tag_name == tag)
        .ok_or_else(|| anyhow::anyhow!("release {tag} was not created"))?;
    if release.assets.len() != 1 {
        bail!(
            "expected one asset after a re-run, got {}",
            release.assets.len()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_tags_filters_and_sorts() {
        let tags = vec![
            "base".to_string(),
            "v1.2.4".to_string(),
            "v1.2.3".to_string(),
        ];
        assert_eq!(version_tags(&tags), vec!["v1.2.3", "v1.2.4"]);
    }

    #[test]
    fn glob_asset_match() {
        assert!(matches("*.tgz", "thing-1.2.3.tgz").unwrap());
        assert!(!matches("*.tgz", "thing-1.2.3.zip").unwrap());
    }
}
