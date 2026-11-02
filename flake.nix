{
  description = "Foundation development environment and production images";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/7a6522c868711d06e3a520bbcdfafe9806f4337f";
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
      packages = nixpkgs.lib.genAttrs [ "aarch64-linux" "x86_64-linux" ] (system:
        let
          pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./services/catalyst/rust-toolchain.toml;
        in import ./infra/packages {
          inherit pkgs;
          craneLib = (crane.mkLib pkgs).overrideToolchain rust;
          revision = self.rev or self.dirtyRev or "unversioned";
        });

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
          default = pkgs.mkShell ({
            packages = with pkgs; [
              rust nodejs_26 (pnpm.override { nodejs-slim = nodejs-slim_26; })
              postgresql_18 process-compose watchexec livekit nats-server python3 bashInteractive
              gitMinimal git-lfs gh ripgrep libarchive pkg-config cmake clang protobuf curl jq ffmpeg-headless dev
            ];
            buildInputs = [ pkgs.openssl ] ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.libiconv ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            OPENSSL_NO_VENDOR = "1";
            LOCALE_ARCHIVE = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux "${pkgs.glibcLocales}/lib/locale/locale-archive";
            shellHook = builtins.readFile ./infra/dev/shell-hook.sh;
          } // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            # The prebuilt V8 the image build links (infra/packages/catalyst.nix), so a dev
            # build neither downloads it nor trusts a restored target cache to still hold it.
            RUSTY_V8_ARCHIVE = "${pkgs.callPackage ./infra/packages/librusty-v8.nix { }}";
          });
        in {
          inherit default;
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
