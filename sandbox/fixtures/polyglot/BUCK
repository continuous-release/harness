load("@prelude//toolchains:demo.bzl", "system_demo_toolchains")

# The release version is injected at parse time from `-c release.version=...`
# so every target and genrule in this build sees the same value.
version = read_config("release", "version", "0.0.0")

rust_library(
    name = "polyglot_lib",
    srcs = [
        "rust/src/contract.rs",
        "rust/src/lib.rs",
        "rust/src/version.rs",
    ],
    crate = "polyglot",
    crate_root = "rust/src/lib.rs",
    edition = "2021",
    env = {"RELEASE_VERSION": version},
)

rust_binary(
    name = "polyglot",
    srcs = ["rust/src/main.rs"],
    crate_root = "rust/src/main.rs",
    edition = "2021",
    deps = [":polyglot_lib"],
)

rust_test(
    name = "rust_test",
    srcs = [
        "rust/src/contract.rs",
        "rust/src/lib.rs",
        "rust/src/version.rs",
    ],
    crate_root = "rust/src/lib.rs",
    edition = "2021",
)

genrule(
    name = "ts_pack",
    srcs = [
        "scripts/build-ts.mjs",
        "ts/package.json",
        "ts/src/contract.js",
        "ts/src/index.js",
        "ts/src/version.js",
    ],
    out = "polyglot.tgz",
    cmd = "RELEASE_VERSION={v} node $SRCDIR/scripts/build-ts.mjs --version {v} --out $OUT".format(v = version),
)

genrule(
    name = "py_wheel",
    srcs = [
        "python/pyproject.toml",
        "python/src/polyglot/__init__.py",
        "python/src/polyglot/contract.py",
        "scripts/build-python.mjs",
    ],
    out = "polyglot.whl",
    cmd = "RELEASE_VERSION={v} node $SRCDIR/scripts/build-python.mjs --version {v} --out $OUT".format(v = version),
)

genrule(
    name = "manifest",
    srcs = [
        ":polyglot",
        ":py_wheel",
        ":ts_pack",
        "contract/contract.json",
        "scripts/manifest.mjs",
    ],
    out = "manifest.json",
    cmd = "node $SRCDIR/scripts/manifest.mjs --version {v} --out $SRCDIR --manifest $OUT --artifact polyglot=$SRCDIR/polyglot --artifact polyglot.tgz=$SRCDIR/polyglot.tgz --artifact polyglot.whl=$SRCDIR/polyglot.whl".format(v = version),
)

genrule(
    name = "verify",
    srcs = [
        ":manifest",
        ":polyglot",
        ":py_wheel",
        ":ts_pack",
        "contract/contract.json",
        "scripts/verify.mjs",
    ],
    out = "verified.txt",
    cmd = "RELEASE_VERSION={v} node $SRCDIR/scripts/verify.mjs --dist $SRCDIR && touch $OUT".format(v = version),
)

genrule(
    name = "release",
    srcs = [
        ":manifest",
        ":polyglot",
        ":py_wheel",
        ":ts_pack",
    ],
    out = ".",
    cmd = "mkdir -p $OUT && cp $SRCDIR/polyglot $OUT/ && cp $SRCDIR/polyglot.tgz $OUT/ && cp $SRCDIR/polyglot.whl $OUT/ && cp $SRCDIR/manifest.json $OUT/",
)
