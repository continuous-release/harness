# continuous-release harness

Live scenarios for the [continuous-release toolkit](https://github.com/continuous-release/toolkit)
and the shared release workflow. Every scenario runs against real GitHub
Actions in the [sandbox](https://github.com/continuous-release/sandbox); there
is no local simulation of the release contract.

## How a scenario runs

1. Reset the sandbox to its `base` tag and delete every `v*` tag and release.
2. Write `scenario.json` and the scenario's commits.
3. Force-push `main` (or open a pull request).
4. Wait for the real workflow, then assert the release, tag, notes, assets,
   and workflow conclusion.
5. Clean up unless `--keep` is set.

`scenario.json` selects the fixture and the setup, test, build, and publish
commands, so one sandbox exercises the same workflow with different behavior:

```json
{
  "system": "node",
  "directory": "fixtures/node",
  "setup": "npm install --ignore-scripts",
  "test": "npm test",
  "build": "mkdir -p dist && npm version \"$VERSION\" --no-git-tag-version --allow-same-version && npm pack --pack-destination dist",
  "publish": "",
  "artifact": "dist/*.tgz"
}
```

## Usage

```sh
npm ci
npm run build

export GITHUB_TOKEN=...          # repo and workflow access
node dist/index.js list
node dist/index.js run first-release
node dist/index.js run --all --timeout 600
```

`--repo owner/name` overrides `SANDBOX_REPO` (default
`continuous-release/sandbox`). `--keep` leaves the last release and tag in place
for inspection.

## Scenarios

| Scenario | What it proves |
| --- | --- |
| `first-release` | No tags starts at `0.1.0` |
| `patch` | A fix bumps patch |
| `minor` | A feature bumps minor |
| `major-bang` | `!` bumps major |
| `major-footer` | `BREAKING CHANGE` bumps major |
| `chore-patch` | Every commit releases; a chore is a patch |
| `release-as` | `Release-As` forces a version |
| `prerelease` | A prerelease version is marked as such |
| `notes-grouping` | Notes group commits and link the compare range |
| `idempotent-rerun` | Re-running the same commit updates the release |
| `existing-tag-at-head` | A tag on HEAD updates instead of bumping |
| `build-failure` | A failed build leaves no release and drops the reservation |
| `publish-failure` | A failed publish leaves no release |
| `missing-artifact` | A missing artifact fails before the release |
| `pr-no-release` | A pull request builds and tests but never releases |

## Seeding the sandbox

The sandbox repo is disposable. To recreate it from `sandbox/`:

```sh
GITHUB_TOKEN=... node scripts/seed.mjs --repo continuous-release/sandbox
```

This copies the template, commits it, and force-moves the `base` tag. The
harness always resets to `base`, so the template is the real source of truth.

## Caveats

- GitHub concurrency is not FIFO: with `cancel-in-progress: false`, a newer
  pending run replaces an older pending run. Scenarios run one at a time and
  wait for completion, so they do not depend on queue order.
- The sandbox is force-pushed, so never point the harness at a repository that
  holds real history.
- The live suite needs a token with `repo` and `workflow` scopes.
