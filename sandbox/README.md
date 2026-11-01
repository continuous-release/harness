# continuous-release sandbox

Live target for the [continuous-release harness](https://github.com/continuous-release/harness).

The harness resets `main` to the `base` tag, writes `scenario.json`, and pushes
a commit. The `release` workflow reads the scenario and delegates to
`continuous-release/.github/.github/workflows/auto-release.yml`, so every
scenario exercises the real toolkit and the real release workflow.

`ci.yml` runs the selected fixture on pull requests. It never releases.
