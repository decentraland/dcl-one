{
  description = "Foundation development environment and production images";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/f9a75f41be3a74eab55d3a7679d34ad6cc0da977";
    rust-overlay = {
      url = "github:oxalica/rust-overlay/42076d1c6a0fb405ed04f9a674e4a8f69de62ba3";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane/v0.24.0";
  };

  outputs = { self, nixpkgs, rust-overlay, crane, ... }:
    let
      systems = [ "aarch64-darwin" "aarch64-linux" "x86_64-linux" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in {
      packages = eachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./services/catalyst/rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain rust;
        in import ./infra/packages/uuav.nix { inherit pkgs craneLib; }
        // (if pkgs.stdenv.hostPlatform.isLinux then import ./infra/packages {
          inherit pkgs craneLib;
          revision = self.rev or self.dirtyRev or "unversioned";
        # The tool environments alone: the macOS CI legs take ffmpeg from here.
        } else import ./infra/packages/tools.nix { inherit pkgs; }));

      checks.x86_64-linux.dev-service = import ./infra/dev/service-test {
        pkgs = import nixpkgs { system = "x86_64-linux"; };
        devShell = self.devShells.x86_64-linux.default;
        inputs = { inherit nixpkgs rust-overlay crane; };
      };

      devShells = eachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./services/catalyst/rust-toolchain.toml;
          dev = pkgs.writeShellScriptBin "foundation-dev" ''
            exec python3 "''${FOUNDATION_DEV_ROOT:?Enter nix develop first}/tooling/dev.py" "$@"
          '';
          environment = {
            buildInputs = [ pkgs.openssl ] ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            OPENSSL_NO_VENDOR = "1";
            shellHook = builtins.readFile ./infra/dev/shell-hook.sh;
          } // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            # The prebuilt V8 the image build links (infra/packages/catalyst.nix), so a dev
            # build neither downloads it nor trusts a restored target cache to still hold it.
            RUSTY_V8_ARCHIVE = "${pkgs.callPackage ./infra/packages/librusty-v8.nix { }}";
          };
          default = pkgs.mkShell (environment // {
            packages = with pkgs; [
              rust nodejs_26 (pnpm.override { nodejs-slim = nodejs-slim_26; })
              postgresql_18 process-compose watchexec livekit nats-server python3 bashInteractive
              gitMinimal git-lfs gh ripgrep libarchive pkg-config cmake clang protobuf curl jq ffmpeg-headless dev
            ];
            LOCALE_ARCHIVE = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux "${pkgs.glibcLocales}/lib/locale/locale-archive";
          });
          toolchain = (builtins.fromTOML (builtins.readFile ./services/catalyst/rust-toolchain.toml)).toolchain;
          ci = pkgs.mkShell (environment // {
            packages = with pkgs; [
              (rust-bin.fromRustupToolchain (toolchain // { profile = "minimal"; }))
              postgresql_18 process-compose nats-server python3 bashInteractive gitMinimal
              pkg-config cmake clang protobuf dev
            ];
            LOCALE_ARCHIVE = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux "${
              pkgs.glibcLocales.override { allLocales = false; locales = [ "C.UTF-8/UTF-8" "en_US.UTF-8/UTF-8" ]; }
            }/lib/locale/locale-archive";
          });
        in {
          inherit default ci;
          # foundation-dev desktop: the Tauri shell (apps/desktop). Linux needs
          # WebKitGTK; libsoup gets TLS from glib-networking.
          desktop = default.overrideAttrs (old: {
            buildInputs = old.buildInputs ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux
              (with pkgs; [ gtk3 webkitgtk_4_1 libsoup_3 glib-networking ]);
            GIO_EXTRA_MODULES = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux
              "${pkgs.glib-networking}/lib/gio/modules";
            WEBKIT_DISABLE_DMABUF_RENDERER = "1";
            FOUNDATION_DEV_DESKTOP_SHELL = "1";
          });
        });
    };
}
