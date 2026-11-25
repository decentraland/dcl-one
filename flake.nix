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
      rustPackages = system:
        let
          pkgs = import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./services/catalyst/rust-toolchain.toml;
        in { inherit pkgs; craneLib = (crane.mkLib pkgs).overrideToolchain rust; };
      explorerRust = rustPackages "x86_64-linux";
      # Three chained layers: third-party crates, Bevy and the crates that use it, the Explorer.
      bevy-explorer = import ./infra/packages/bevy-explorer.nix explorerRust;
    in {
      packages = nixpkgs.lib.genAttrs [ "aarch64-linux" "x86_64-linux" ] (system:
        import ./infra/packages (rustPackages system // {
          revision = self.rev or self.dirtyRev or "unversioned";
        }) // import ./infra/packages/uuav.nix (rustPackages system)) // {
          # The tool environments: the macOS CI legs take ffmpeg from here. `cast-bot` too, for a
          # local node's CAST_BOT_BIN, and the message router for a local node's community chat
          # (not built in CI). The UUAV runtime set builds on macOS as well.
          aarch64-darwin = let
            darwin = rustPackages "aarch64-darwin";
            catalyst = import ./infra/packages/catalyst.nix darwin;
            cast-bot = import ./infra/packages/cast-bot.nix darwin;
          in import ./infra/packages/tools.nix { pkgs = import nixpkgs { system = "aarch64-darwin"; }; } // {
            inherit cast-bot;
            message-router = import ./infra/packages/message-router.nix (darwin // { inherit catalyst cast-bot; });
          } // import ./infra/packages/uuav.nix darwin;
        };

      # `nix build .#bevy-explorer` and `.#bevy-explorer.tests`. An evaluation of the Explorer
      # builds its patched sources, and `nix flake check` evaluates every attribute of
      # `packages` and `checks`, so the Explorer is in neither.
      legacyPackages.x86_64-linux = {
        inherit bevy-explorer;
        explorer = import ./infra/packages/explorer.nix { inherit (explorerRust) pkgs; inherit bevy-explorer; };
        bevy-explorer-deps = bevy-explorer.deps;
        bevy-explorer-bevy = bevy-explorer.bevy;
      };

      checks.x86_64-linux.dev-service = import ./infra/dev/service-test {
        pkgs = import nixpkgs { system = "x86_64-linux"; };
        devShell = self.devShells.x86_64-linux.default;
        inputs = { inherit nixpkgs rust-overlay crane; };
      };

      devShells = eachSystem (system:
        let
          inherit (rustPackages system) pkgs;
          inherit (pkgs) lib;
          linux = pkgs.stdenv.hostPlatform.isLinux;
          built = self.packages.${system};
          rust = pkgs.rust-bin.fromRustupToolchainFile ./services/catalyst/rust-toolchain.toml;
          nodeTools = [ pkgs.nodejs_26 (pkgs.callPackage ./infra/packages/pnpm.nix { }) ];
          nodeEnvironment = { PNPM = "pnpm"; pnpm_config_manage_package_manager_versions = "false"; };
          browserTools = lib.optionals linux [ pkgs.chromium ];
          browserEnvironment = lib.optionalAttrs linux (let chromium = lib.getExe pkgs.chromium; in {
            BROWSER_PATH = chromium; CHROME_PATH = chromium; CHROMIUM_BIN = chromium;
          });
          playwrightEnvironment = consumer: lib.optionalAttrs linux {
            PLAYWRIGHT_BROWSERS_PATH = "${import ./infra/packages/playwright-browsers.nix { inherit pkgs consumer; }}";
            PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
          };
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
          default = pkgs.mkShell (environment // nodeEnvironment // browserEnvironment // {
            packages = with pkgs; nodeTools ++ browserTools ++ [
              rust
              postgresql_18 process-compose watchexec livekit livekit-ingress valkey nats-server python3 bashInteractive
              gitMinimal git-lfs gh ripgrep libarchive pkg-config cmake clang protobuf curl jq ffmpeg-headless dev
              procps which rsync unzip
            ];
            SDK_PROTOC = "${pkgs.protobuf_21}/bin/protoc";
            LOCALE_ARCHIVE = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux "${pkgs.glibcLocales}/lib/locale/locale-archive";
          });
          toolchain = (builtins.fromTOML (builtins.readFile ./services/catalyst/rust-toolchain.toml)).toolchain;
          ci = pkgs.mkShell (environment // {
            packages = with pkgs; [
              (rust-bin.fromRustupToolchain (toolchain // { profile = "minimal"; }))
              postgresql_18 process-compose nats-server python3 bashInteractive gitMinimal
              pkg-config cmake clang protobuf dev procps
            ];
            LOCALE_ARCHIVE = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux "${
              pkgs.glibcLocales.override { allLocales = false; locales = [ "C.UTF-8/UTF-8" "en_US.UTF-8/UTF-8" ]; }
            }/lib/locale/locale-archive";
          });
          # The web explorer's loop, `just wasm` in rendering/bevy-explorer/source: the nightly its
          # justfile pins, and wasm-bindgen at that workspace's Cargo.lock version, so wasm-pack
          # downloads neither it nor wasm-opt. No services: the shell hook is the default shell's.
          # Node 26 has no corepack, so the shell carries pnpm and the justfile reads PNPM. This
          # pnpm must not fetch the version a package.json pins: that binary cannot start on NixOS.
          wasm = pkgs.mkShell (nodeEnvironment // browserEnvironment // playwrightEnvironment "react-web" // {
            packages = with pkgs; nodeTools ++ browserTools ++ [
              (rust-bin.nightly."2026-04-15".default.override {
                targets = [ "wasm32-unknown-unknown" ];
                extensions = [ "rust-src" ];
              })
              just wasm-pack wasm-bindgen-cli_0_2_114 binaryen python3 gitMinimal git-lfs
              pkg-config cmake clang protobuf
            ] ++ lib.optionals stdenv.hostPlatform.isLinux [ udev alsa-lib ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          });
          # The native Explorer's loop, `cargo build` and `just native-debug` in
          # rendering/bevy-explorer/source: the toolchain, the build tools and the libraries of the
          # Nix package (infra/packages/bevy-explorer-native.nix). No services. Linux compiles with
          # the package's clang; on x86_64 the prebuilt V8 and CEF come from the store, so no build
          # script downloads them and `just setup-cef` is not needed. macOS compiles and links with
          # Apple's toolchain (Xcode command line tools), which has the SDK the engine needs.
          native = import ./infra/packages/bevy-explorer-native.nix { inherit pkgs; };
          explorerHook = builtins.readFile ./infra/dev/explorer-shell-hook.sh;
          explorerTools = with pkgs; native.tools ++ nodeTools ++ [ rust just python3 gitMinimal git-lfs taplo procps ];
          explorerNode = nodeEnvironment // { LK_CUSTOM_WEBRTC = "${built.cast-bot.webrtc}"; };
          explorer =
            if pkgs.stdenv.hostPlatform.isDarwin then pkgs.mkShellNoCC (explorerNode // {
              packages = explorerTools;
              buildInputs = [ pkgs.ffmpeg_6-headless ];
              shellHook = explorerHook;
            })
            else (pkgs.mkShell.override { inherit (native) stdenv; }) (explorerNode // browserEnvironment // {
              packages = explorerTools ++ browserTools ++ [ pkgs.rustPlatform.bindgenHook pkgs.xvfb-run ];
              buildInputs = native.libraries;
              VK_ADD_DRIVER_FILES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.${pkgs.stdenv.hostPlatform.uname.processor}.json";
              WGPU_BACKEND = "vulkan";
              # What the client loads at run time and no binary names: the window system, Vulkan, audio.
              EXPLORER_RUNTIME_LIBRARIES = pkgs.lib.makeLibraryPath native.runtimeLibraries;
              shellHook = explorerHook;
            } // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
              RUSTY_V8_ARCHIVE = "${native.v8}";
              CEF_PATH = "${native.cef}";
            });
          creator-hub = pkgs.mkShell (nodeEnvironment // playwrightEnvironment "creator-hub" // {
            packages = with pkgs; nodeTools ++ [ python3 gitMinimal git-lfs unzip procps ]
              ++ lib.optionals linux [ xvfb-run ];
            CREATOR_HUB_PROTOC = "${pkgs.protobuf_21}/bin/protoc";
            ELECTRON_OVERRIDE_DIST_PATH = let electron = import ./infra/packages/electron.nix { inherit pkgs; }; in
              if linux then "${electron}/libexec/electron" else "${electron}/Applications";
            npm_config_nodedir = "${pkgs.nodejs_26}";
            shellHook = lib.optionalString linux ''
              E2E_APP_PATH="$(git rev-parse --show-toplevel)/apps/creator-hub/packages/creator-hub/dist/linux-unpacked/creator-hub"
              export E2E_APP_PATH
            '';
          });
          uuavToolchain = let toolchain = (builtins.fromTOML (builtins.readFile ./library/uuav/rust-toolchain.toml)).toolchain; in
            pkgs.rust-bin.fromRustupToolchain {
              inherit (toolchain) channel components;
              targets = lib.optionals (system == "x86_64-linux") [ "x86_64-pc-windows-gnu" ];
            };
          uuavFfmpeg = built.uuav-ffmpeg;
          uuav = pkgs.mkShell ({
            packages = with pkgs; [ uuavToolchain gitMinimal git-lfs python3 rustPlatform.bindgenHook ]
              ++ lib.optionals linux [ patchelf ]
              ++ lib.optionals (system == "x86_64-linux") [
                (buildEnv { name = "mingw-w64-cross-linker"; paths = [ pkgsCross.mingwW64.stdenv.cc ]; pathsToLink = [ "/bin" ]; })
              ];
            buildInputs = [ uuavFfmpeg ];
            FFMPEG_DIR = "${uuavFfmpeg}";
          } // lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
            DYLD_FALLBACK_LIBRARY_PATH = "${uuavFfmpeg}/lib";
          });
        in {
          inherit default ci wasm explorer creator-hub uuav;
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
