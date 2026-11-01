# continuous-release harness

Release-contract scenarios for the [continuous-release toolkit](https://github.com/continuous-release/toolkit)
and the shared release workflow. The same scenarios run two ways:

- **Local**: a real git repository plus an in-memory GitHub API, driven by the
  real `cr` binary. Seconds per scenario, no network.
- **Live**: real GitHub Actions in a disposable sandbox repository. Full
  fidelity, sharded across a pool of sandboxes.

Both modes use the same scenario definitions and the same assertions.

## How a scenario runs

1. Reset the sandbox to its `base` tag and delete every `v*` tag and release.
2. Write `scenario.json` and the scenario's commits.
3. Push `main` (or open a pull request).
4. Run the release contract (local) or wait for the real workflow (live), then
   assert the release, tag, notes, assets, and workflow conclusion.
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
cargo build --release --locked

# Fast feedback: local git + an in-memory GitHub API, the same scenarios and
# assertions as live.
CR_BIN=../toolkit/target/release/cr cargo run --release --locked -- run --all

# Fidelity: real GitHub Actions in the sandbox.
export GITHUB_TOKEN=...          # repo and workflow access
cargo run --release --locked -- run --all --live
```

`--repo owner/name` overrides `SANDBOX_REPO` (default
`continuous-release/sandbox`). `--keep` skips live cleanup and keeps the local
repositories for inspection. `--shard 0/4` selects every fourth scenario, which
is how the live suite runs four sandboxes in parallel.

The harness is Rust like the toolkit: it has no Node runtime and the fake
GitHub is a real HTTP server, so it exercises the same binary users run.

## CI

- **Every pull request** runs the local suite. It is the merge gate and takes
  seconds.
- **Every same-repository pull request and every push to `main`** runs the live
  suite in four shards against `sandbox-0` … `sandbox-3`. The workflow-level
  concurrency group serializes suites so shards never share a sandbox.
- **Fork pull requests** do not receive the sandbox token, so they run the local
  suite only. Pushing the branch to this repository runs the live suite without
  exposing the token.
- **Every release** is verified automatically: the tag target, published state,
  asset count, checksum, and the binary's reported version. The toolkit's own
  release workflow does this in its `verify` job; the shared workflow also
  checks that the release is published and has assets.

The live pool is disposable. `harness seed` recreates any sandbox from the
template, including the `base` tag.

## Scenarios

| Scenario | What it proves |
| --- | --- |
| `first-release` | No tags starts at `0.1.0` |
| `polyglot-release` | One contract releases a Rust binary, npm tarball, and Python wheel |
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
| `publish-failure` | A failed publish keeps the tag but creates no release |
| `publish-failure-rerun` | A repaired publish reuses the reserved tag |
| `tag-collision` | An unrelated tag supersedes the run |
| `missing-artifact` | A missing artifact fails before the release |
| `pr-no-release` | A pull request builds and tests but never releases |

## Seeding the sandbox

The sandbox repos are disposable. To recreate one from `sandbox/`:

```sh
GITHUB_TOKEN=... cargo run --release --locked -- seed \
  --repo continuous-release/sandbox-0
```

This copies the template, commits it, and force-moves the `base` tag. The
harness always resets to `base`, so the template is the real source of truth.
Repeat for `sandbox-1` … `sandbox-3` when the pool needs a reset.

The template includes `fixtures/polyglot`, a snapshot of
`continuous-release/polyglot` v0.1.2. Refresh it with
`scripts/sync-polyglot-fixture.sh ../polyglot`. The harness runs its canonical
fast path and verifies all three artifacts; the polyglot repository's CI matrix
covers the four build-system adapters.

## Caveats

- GitHub concurrency is not FIFO: with `cancel-in-progress: false`, a newer
  pending run replaces an older pending run. The live suite serializes whole
  runs and waits for completion, so shards do not depend on queue order.
- The sandbox is force-pushed, so never point the harness at a repository that
  holds real history.
- The live suite needs a token with `repo` and `workflow` scopes.
- The fake GitHub implements only the subset `cr` uses; it is a contract test
  double, not a general GitHub emulator.
