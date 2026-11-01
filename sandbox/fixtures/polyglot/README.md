# polyglot

L2 polyglot fixture for [continuous-release](https://github.com/continuous-release):
one source tree, four build-system adapters, one release manifest.

The contract is a single JSON document. Codegen turns it into Rust, TypeScript,
and Python constants. The release build produces three artifacts that must all
report the same version:

| Artifact | Language | Built by |
| --- | --- | --- |
| `polyglot` | Rust | `cargo` |
| `polyglot.tgz` | TypeScript | `npm pack` |
| `polyglot.whl` | Python | `pip wheel` |
| `manifest.json` | — | `scripts/manifest.mjs` |

`scripts/verify.mjs` checks the manifest hashes, runs the Rust binary, reads the
npm package version, and reads the wheel metadata.

## Adapters

| Adapter | Entry point | What it exercises |
| --- | --- | --- |
| Nix | `nix build .#release` | Hermetic multi-language build with a static musl Rust binary |
| Bazel | `bazel build //:verify` | Explicit Rust target + generated-artifact graph |
| Buck2 | `buck2 build //:release -c release.version=...` | Config-driven version + genrules |
| Nx | `npx nx run polyglot:finalize` | Task graph, affected builds, local cache |

The release workflow uses the Nx adapter as the canonical build. Nx uses the
runner's `cargo` and Python, so the released Rust binary links against the
runner's glibc and runs in the release-verify job and on normal Linux hosts.
The CI matrix runs all four adapters and verifies the same manifest in each.

Nix is the hermetic comparison and now builds the Rust binary for
`x86_64-unknown-linux-musl` with a pinned rust-overlay toolchain, producing a
static-pie artifact. The Nix CI job executes it on Ubuntu just like the other
adapters.

## Local

```bash
# Canonical release path (Nx)
npm ci
RELEASE_VERSION=0.1.0 npx nx run polyglot:finalize

# Fast path without Nx
RELEASE_VERSION=0.1.0 bash scripts/build-release.sh

# Nix (version comes from version.txt)
nix build .#release --print-build-logs

# Bazel
RELEASE_VERSION=0.1.0 bazel build //:verify

# Buck2
buck2 build //:release -c release.version=0.1.0
```

## Version

The release version comes from `RELEASE_VERSION`, falling back to `version.txt`;
Nix reads `version.txt` directly. Every adapter embeds the version in the Rust
binary at compile time, and `verify.mjs` runs the binary without
`RELEASE_VERSION` so the artifact must report its own version. `manifest.json`
is authoritative; `verify.mjs` checks every artifact against it.

## Why the generated files are committed

Bazel and Buck2 need stable source paths, and every adapter should compile the
same inputs. `scripts/codegen.mjs --check` fails CI when `contract/contract.json`
changed without regenerating the Rust, TypeScript, and Python constants.

## Contract

See [continuous-release/.github](https://github.com/continuous-release/.github)
for the shared workflow, version rules, and release notes format.
