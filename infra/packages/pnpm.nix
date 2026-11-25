{ lib, stdenvNoCC, fetchurl, nodejs-slim_26 }:
let
  version = "12.21.0";
  releases = {
    x86_64-linux = { target = "linux-x64-musl"; hash = "sha256-bbzZ6o7Te1rL4nkVmtDtOkrGFPXXrmLVulHTKoWlidc="; };
    aarch64-linux = { target = "linux-arm64-musl"; hash = "sha256-T67aqjM7RNlAzl44wiPoKRJK/9vlptP2lKad2XmlqNg="; };
    aarch64-darwin = { target = "darwin-arm64"; hash = "sha256-XE1zAY4A3WAlMc5xRiVZT7aTy+EQ5nGd8rBDsK5iqxo="; };
  };
  release = releases.${stdenvNoCC.hostPlatform.system};
  projectsPin = (lib.importJSON ../../apps/sites/package.json).packageManager or null;
in
assert lib.assertMsg (projectsPin == null || lib.head (lib.splitString "+" projectsPin) == "pnpm@${version}")
  "infra/packages/pnpm.nix has pnpm ${version}, but the projects pin ${projectsPin}";
stdenvNoCC.mkDerivation {
  pname = "pnpm";
  inherit version;
  src = fetchurl {
    url = "https://github.com/pnpm/pnpm/releases/download/v${version}/pnpm-${release.target}.tar.gz";
    inherit (release) hash;
  };
  sourceRoot = ".";
  dontStrip = true;
  installPhase = ''
    mkdir -p $out/libexec/pnpm $out/bin
    cp -r pnpm dist $out/libexec/pnpm/
    ln -s ../libexec/pnpm/pnpm $out/bin/pnpm
  '';
  passthru.nodejs-slim = nodejs-slim_26;
  meta.mainProgram = "pnpm";
}
