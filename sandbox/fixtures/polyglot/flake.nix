{
  description = "polyglot — one source tree, four build-system adapters";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      version = nixpkgs.lib.removeSuffix "\n" (builtins.readFile ./version.txt);
    in {
      packages = forAllSystems (pkgs:
        let
          python = pkgs.python3.withPackages (ps: [ ps.pip ps.setuptools ps.wheel ]);
          rust = (pkgs.extend rust-overlay.overlays.default).rust-bin.stable.latest.default.override {
            targets = [ "x86_64-unknown-linux-musl" ];
          };
          muslCc = pkgs.pkgsMusl.stdenv.cc;
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let base = baseNameOf path;
              in !(builtins.elem base [ "target" "dist" "dist-buck" "node_modules" "buck-out" "result" ".nx" ])
                && builtins.match "bazel-.*" base == null;
          };
          release = pkgs.stdenv.mkDerivation {
            pname = "polyglot";
            inherit version src;

            nativeBuildInputs = [
              pkgs.nodejs
              rust
              muslCc
              python
              pkgs.unzip
            ];

            # The manifest hashes the artifacts, so do not let fixup rewrite them
            # after they are measured.
            dontFixup = true;
            dontStrip = true;

            buildPhase = ''
              export HOME=$TMPDIR
              export CARGO_HOME=$TMPDIR/cargo
              export npm_config_cache=$TMPDIR/npm
              export POLYGLOT_OFFLINE=1
              export POLYGLOT_RUST_TARGET=x86_64-unknown-linux-musl
              export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=${muslCc}/bin/cc
              export CC_x86_64_unknown_linux_musl=${muslCc}/bin/cc
              export RELEASE_VERSION=${version}
              bash scripts/build-release.sh
            '';

            installPhase = ''
              mkdir -p $out
              cp -r dist/. $out/
            '';

            meta.description = "Polyglot release artifacts";
          };
        in {
          inherit release;
          default = release;
        });

      checks = forAllSystems (pkgs: {
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.release;
      });
    };
}
