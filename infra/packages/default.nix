{ pkgs, craneLib, revision }:
let
  catalyst = import ./catalyst.nix { inherit pkgs craneLib; };
  cast-bot = import ./cast-bot.nix { inherit pkgs craneLib; };
  message-router = import ./message-router.nix { inherit pkgs craneLib catalyst cast-bot; };
  scene-cli = import ./scene-cli.nix { inherit pkgs craneLib; inherit (catalyst) vendorGit; };
  playWeb = pkgs.callPackage ./play.nix { };
  sites = pkgs.callPackage ./sites.nix { inherit playWeb; };
  # nginx 1.30.7 (stable), ahead of the flake's nixpkgs pin, which carries 1.30.6.
  nginx = pkgs.nginx.overrideAttrs (_: rec {
    version = "1.30.7";
    src = pkgs.fetchurl {
      url = "https://nginx.org/download/nginx-${version}.tar.gz";
      hash = "sha256-JWW1yDcFDpWPcPsPc8SOK8pFQq00n9NW9G+wxyYQvG4=";
    };
  });
  gateway = pkgs.callPackage ../gateway/package.nix { inherit nginx; };
in {
  inherit catalyst cast-bot message-router scene-cli sites gateway;
  catalyst-deps = catalyst.cargoArtifacts;
  catalyst-vendor = catalyst.cargoVendorDir;
  sites-app = sites.app;
  sites-deps = sites.pnpmDeps;
  sites-runtime-deps = sites.runtimeDeps;
  sites-toolchain = sites.toolchain;
  image-tools = pkgs.buildEnv {
    name = "foundation-image-tools";
    paths = [ pkgs.skopeo pkgs.jq pkgs.python3 pkgs.curl ];
  };
} // import ./tools.nix { inherit pkgs; }
  // import ../containers/images.nix { inherit pkgs catalyst cast-bot message-router sites gateway nginx revision; }
