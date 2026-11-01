//! Scenario definitions. The per-run build commands stay data-driven in
//! `scenario.json`; this file only describes what each scenario sets up and
//! what the release contract must produce.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SandboxConfig {
    pub system: String,
    pub directory: String,
    pub setup: String,
    pub test: String,
    pub build: String,
    pub publish: String,
    pub artifact: String,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            system: "node".into(),
            directory: "fixtures/node".into(),
            setup: "npm install --ignore-scripts".into(),
            test: "npm test".into(),
            build: concat!(
                "mkdir -p dist && npm version \"$VERSION\" --no-git-tag-version ",
                "--allow-same-version && npm pack --pack-destination dist"
            )
            .into(),
            publish: String::new(),
            artifact: "dist/*.tgz".into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigOverride {
    pub system: Option<String>,
    pub directory: Option<String>,
    pub setup: Option<String>,
    pub test: Option<String>,
    pub build: Option<String>,
    pub publish: Option<String>,
    pub artifact: Option<String>,
}

impl ConfigOverride {
    pub fn is_some(&self) -> bool {
        self.system.is_some()
            || self.directory.is_some()
            || self.setup.is_some()
            || self.test.is_some()
            || self.build.is_some()
            || self.publish.is_some()
            || self.artifact.is_some()
    }
}

impl SandboxConfig {
    pub fn merged(&self, overrides: &ConfigOverride) -> Self {
        Self {
            system: overrides
                .system
                .clone()
                .unwrap_or_else(|| self.system.clone()),
            directory: overrides
                .directory
                .clone()
                .unwrap_or_else(|| self.directory.clone()),
            setup: overrides
                .setup
                .clone()
                .unwrap_or_else(|| self.setup.clone()),
            test: overrides.test.clone().unwrap_or_else(|| self.test.clone()),
            build: overrides
                .build
                .clone()
                .unwrap_or_else(|| self.build.clone()),
            publish: overrides
                .publish
                .clone()
                .unwrap_or_else(|| self.publish.clone()),
            artifact: overrides
                .artifact
                .clone()
                .unwrap_or_else(|| self.artifact.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommitSpec {
    pub message: &'static str,
    pub body: Option<&'static str>,
    pub files: Vec<(&'static str, &'static str)>,
}

impl CommitSpec {
    fn message(message: &'static str) -> Self {
        Self {
            message,
            body: None,
            files: Vec::new(),
        }
    }

    fn body(message: &'static str, body: &'static str) -> Self {
        Self {
            message,
            body: Some(body),
            files: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkflowExpectation {
    #[default]
    Success,
    Failure,
}

#[derive(Clone, Debug, Default)]
pub struct Expectation {
    pub workflow: Option<WorkflowExpectation>,
    pub release: Option<bool>,
    pub tag: Option<&'static str>,
    pub prerelease: Option<bool>,
    pub assets: Vec<&'static str>,
    pub notes_include: Vec<&'static str>,
    pub notes_exclude: Vec<&'static str>,
    /// Exact version tags expected after the run; defaults to initial_tags.
    pub tags: Option<Vec<&'static str>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Trigger {
    #[default]
    Push,
    Dispatch,
    PullRequest,
}

#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
    pub initial_tags: Vec<&'static str>,
    /// Tags on unrelated commits, as a concurrent release would leave them.
    pub external_tags: Vec<&'static str>,
    pub commits: Vec<CommitSpec>,
    pub config: ConfigOverride,
    pub trigger: Trigger,
    pub rerun: bool,
    /// Dispatch inputs for the re-run, such as a repaired publish command.
    pub rerun_inputs: Vec<(&'static str, &'static str)>,
    pub expect: Expectation,
    pub rerun_expect: Option<Expectation>,
}

fn tag(tag: &'static str) -> Expectation {
    Expectation {
        tag: Some(tag),
        release: Some(true),
        ..Expectation::default()
    }
}

fn tagged(tag_name: &'static str) -> Expectation {
    Expectation {
        assets: vec!["*.tgz"],
        ..tag(tag_name)
    }
}

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "first-release",
            description: "With no tags, the first feature starts at 0.1.0.",
            commits: vec![CommitSpec::message("feat: add the first feature")],
            expect: Expectation {
                notes_include: vec!["### Features", "add the first feature"],
                ..tagged("v0.1.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "polyglot-release",
            description: "One contract releases a Rust binary, npm tarball, and Python wheel.",
            commits: vec![CommitSpec::message("feat: release the polyglot fixture")],
            config: ConfigOverride {
                system: Some("node".into()),
                directory: Some("fixtures/polyglot".into()),
                setup: Some(String::new()),
                test: Some(String::new()),
                build: Some("bash scripts/build-release.sh".into()),
                publish: Some(String::new()),
                artifact: Some("dist/*".into()),
            },
            expect: Expectation {
                assets: vec!["manifest.json", "polyglot", "polyglot.tgz", "polyglot.whl"],
                notes_include: vec!["### Features", "release the polyglot fixture"],
                ..tag("v0.1.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "patch",
            description: "A fix after v1.2.3 bumps the patch version.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            expect: Expectation {
                notes_include: vec!["### Bug Fixes", "correct the timeout"],
                ..tagged("v1.2.4")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "minor",
            description: "A feature after v1.2.3 bumps the minor version.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("feat: add a feature")],
            expect: Expectation {
                notes_include: vec!["### Features", "add a feature"],
                ..tagged("v1.3.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "major-bang",
            description: "A bang on the commit type bumps the major version.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("feat!: drop the old API")],
            expect: Expectation {
                notes_include: vec!["### ⚠ BREAKING CHANGES", "drop the old API"],
                ..tagged("v2.0.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "major-footer",
            description: "A BREAKING CHANGE footer bumps the major version.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::body(
                "fix: adjust the API",
                "BREAKING CHANGE: removed the old flag",
            )],
            expect: Expectation {
                notes_include: vec!["### ⚠ BREAKING CHANGES", "adjust the API"],
                ..tagged("v2.0.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "chore-patch",
            description: "Every releasable commit releases; a chore is a patch.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("chore: tidy the repository")],
            expect: Expectation {
                notes_include: vec!["### Chores", "tidy the repository"],
                ..tagged("v1.2.4")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "release-as",
            description: "Release-As forces an exact version.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::body(
                "chore: force the release",
                "Release-As: 3.4.5",
            )],
            expect: tagged("v3.4.5"),
            ..Scenario::default()
        },
        Scenario {
            name: "prerelease",
            description: "A prerelease version marks the release as a prerelease.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::body(
                "chore: release candidate",
                "Release-As: 1.3.0-rc.1",
            )],
            expect: Expectation {
                prerelease: Some(true),
                ..tagged("v1.3.0-rc.1")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "notes-grouping",
            description: "Notes group commits and link the compare range.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![
                CommitSpec::message("feat: add a feature"),
                CommitSpec::message("fix: correct a bug"),
                CommitSpec::message("docs: explain the feature"),
            ],
            expect: Expectation {
                notes_include: vec![
                    "### Features",
                    "### Bug Fixes",
                    "### Documentation",
                    "compare/v1.2.3...v1.3.0",
                ],
                ..tagged("v1.3.0")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "idempotent-rerun",
            description: "Re-running the same commit updates the release and keeps the tag.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            rerun: true,
            expect: tagged("v1.2.4"),
            ..Scenario::default()
        },
        Scenario {
            name: "existing-tag-at-head",
            description: "A tag already on HEAD updates that release instead of bumping.",
            initial_tags: vec!["v1.2.3"],
            trigger: Trigger::Dispatch,
            expect: tagged("v1.2.3"),
            ..Scenario::default()
        },
        Scenario {
            name: "build-failure",
            description: "A failed build leaves no release and no new tag.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            config: ConfigOverride {
                build: Some("exit 1".into()),
                ..ConfigOverride::default()
            },
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Failure),
                release: Some(false),
                ..Expectation::default()
            },
            ..Scenario::default()
        },
        Scenario {
            name: "publish-failure",
            description: "A failed publish keeps the reserved tag but creates no release.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            config: ConfigOverride {
                publish: Some("exit 1".into()),
                ..ConfigOverride::default()
            },
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Failure),
                release: Some(false),
                tags: Some(vec!["v1.2.3", "v1.2.4"]),
                ..Expectation::default()
            },
            ..Scenario::default()
        },
        Scenario {
            name: "publish-failure-rerun",
            description: "A repaired publish re-runs the same tag and creates one release.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            config: ConfigOverride {
                publish: Some("exit 1".into()),
                ..ConfigOverride::default()
            },
            rerun: true,
            rerun_inputs: vec![("publish-override", ":")],
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Failure),
                release: Some(false),
                tags: Some(vec!["v1.2.3", "v1.2.4"]),
                ..Expectation::default()
            },
            rerun_expect: Some(Expectation {
                notes_include: vec!["correct the timeout"],
                ..tagged("v1.2.4")
            }),
            ..Scenario::default()
        },
        Scenario {
            name: "tag-collision",
            description: "A tag reserved by an unrelated commit supersedes this run.",
            initial_tags: vec!["v1.2.3"],
            external_tags: vec!["v1.2.4"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Success),
                release: Some(false),
                tags: Some(vec!["v1.2.3", "v1.2.4"]),
                ..Expectation::default()
            },
            ..Scenario::default()
        },
        Scenario {
            name: "missing-artifact",
            description: "A missing artifact fails before the release is created.",
            initial_tags: vec!["v1.2.3"],
            commits: vec![CommitSpec::message("fix: correct the timeout")],
            config: ConfigOverride {
                artifact: Some("dist/nope-*.tgz".into()),
                ..ConfigOverride::default()
            },
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Failure),
                release: Some(false),
                tags: Some(vec!["v1.2.3", "v1.2.4"]),
                ..Expectation::default()
            },
            ..Scenario::default()
        },
        Scenario {
            name: "pr-no-release",
            description: "A pull request builds and tests, but never releases.",
            trigger: Trigger::PullRequest,
            commits: vec![CommitSpec::message("feat: propose a feature")],
            expect: Expectation {
                workflow: Some(WorkflowExpectation::Success),
                release: Some(false),
                ..Expectation::default()
            },
            ..Scenario::default()
        },
    ]
}

pub fn rerun_input<'a>(scenario: &'a Scenario, key: &str) -> Option<&'a str> {
    scenario
        .rerun_inputs
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| *value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_names_are_unique() {
        let all = scenarios();
        let mut names: Vec<&str> = all.iter().map(|scenario| scenario.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
    }

    #[test]
    fn every_release_scenario_has_a_tag() {
        for scenario in scenarios() {
            if scenario.expect.release != Some(false) {
                assert!(scenario.expect.tag.is_some(), "{}", scenario.name);
            }
        }
    }
}
