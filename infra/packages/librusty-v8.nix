{ lib, stdenv, fetchurl }:
let
  # Must match the exact v8 pin in services/catalyst/Cargo.toml.
  version = "152.2.0";
  hashes = {
    x86_64-linux = "sha256-tmg+mvy3f72MssOs9F001SApqyFrMyUiyVF5wS8P7Tw=";
    aarch64-linux = "sha256-xAGWu/GCUy5YP2W32vAZb9uzNU5vrFmzHQU5wg+THz0=";
  };
  archive = fetchurl {
    name = "librusty_v8-${version}.a.gz";
    url = "https://github.com/denoland/rusty_v8/releases/download/v${version}/librusty_v8_release_${stdenv.hostPlatform.rust.rustcTarget}.a.gz";
    hash = hashes.${stdenv.hostPlatform.system};
    meta.sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
in stdenv.mkDerivation {
  pname = "librusty_v8";
  inherit version;
  dontUnpack = true;
  installPhase = ''gzip -dc ${archive} > "$out"'';
}
