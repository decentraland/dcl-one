# Tool environments the Bevy Explorer workflow takes from the flake, on Linux and macOS alike
# (.github/actions/bevy-explorer-tools delivers them from the Actions cache).
{ pkgs }:
let
  bevy-tools = pkgs.buildEnv {
    name = "bevy-explorer-tools";
    paths = [ pkgs.protobuf pkgs.taplo pkgs.wasm-pack ];
  };
in {
  inherit bevy-tools;
  # The ffmpeg 6 libraries the av crate links on macOS: headers and pkg-config files from the
  # dev output, dylibs from lib.
  bevy-ffmpeg = pkgs.buildEnv {
    name = "bevy-explorer-ffmpeg";
    paths = [ pkgs.ffmpeg_6-headless.dev pkgs.ffmpeg_6-headless.lib ];
  };
} // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
  bevy-web-tools = pkgs.buildEnv {
    name = "bevy-explorer-web-tools";
    paths = [ pkgs.protobuf pkgs.llvmPackages_21.clang-unwrapped pkgs.pkg-config
      pkgs.alsa-lib.dev pkgs.udev.dev ];
  };
}
