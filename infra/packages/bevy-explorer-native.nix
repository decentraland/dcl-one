# What a native build of the Explorer needs beside Cargo. The package (bevy-explorer.nix) and the
# dev shell (`nix develop .#explorer`) take it from here. Nothing here reads a patched source, so
# an evaluation builds nothing.
{ pkgs }:
let
  inherit (pkgs) lib;
  lock = builtins.fromTOML (builtins.readFile ../../rendering/bevy-explorer/source/Cargo.lock);
  lockVersion = name: (lib.findSingle (package: package.name == name)
    (throw "Cargo.lock has no package ${name}") (throw "Cargo.lock has several packages ${name}") lock.package).version;

  # What the build scripts download in a cargo build. Each is passed the way its build script
  # accepts. The versions come from the files that hold them. The name of each download carries
  # its version, so a new version never takes the old file by its hash.
  binary = args: pkgs.fetchurl (args // { meta.sourceProvenance = [ lib.sourceTypes.binaryNativeCode ]; });
  v8 = pkgs.callPackage ./librusty-v8.nix {
    v8Version = lockVersion "v8";
    v8Hashes.x86_64-linux = "sha256-WGn9twcbHyHyAKl86X0gElh34PMc2ALtmd4sU/SIsGw=";
  };
  # The cef-dll-sys version is <crate version>+<CEF version>; the archive name
  # adds the commit and the Chromium version of that CEF build.
  cefVersion = lib.last (lib.splitString "+" (lockVersion "cef-dll-sys"));
  cefArchive = {
    type = "minimal";
    name = "cef_binary_${cefVersion}+g465474a+chromium-139.0.7258.139_linux64_minimal.tar.bz2";
    sha1 = "e23dfadfb4ae8b3551d234d973ff8cd7d7f8469c";
  };
  # The layout `export-cef-dir` writes, with the record cef-dll-sys checks.
  cef = pkgs.stdenv.mkDerivation {
    pname = "cef-minimal";
    version = cefVersion;
    src = binary {
      name = "cef-${cefVersion}-linux64-minimal.tar.bz2";
      url = "https://cef-builds.spotifycdn.com/${lib.replaceStrings [ "+" ] [ "%2B" ] cefArchive.name}";
      hash = "sha256-NettNnBOlIMRHZFHGiQ+atRfSmwPfl1PpYbu+nGAOe4=";
    };
    nativeBuildInputs = [ pkgs.autoPatchelfHook ];
    buildInputs = with pkgs; [
      alsa-lib at-spi2-atk cairo cups dbus expat glib libdrm libgbm libx11 libxcb libxcomposite
      libxdamage libxext libxfixes libxkbcommon libxrandr nspr nss pango stdenv.cc.cc.lib udev
    ];
    dontConfigure = true;
    dontBuild = true;
    dontStrip = true;
    installPhase = ''
      mkdir -p $out
      cp -r Release/. Resources/. $out/
      cp ${builtins.toFile "archive.json" (builtins.toJSON cefArchive)} $out/archive.json
    '';
  };
in {
  inherit binary v8 cef;
  # The CI test job compiles C and C++ with clang 21.
  stdenv = pkgs.llvmPackages_21.stdenv;
  tools = with pkgs; [ pkg-config cmake ninja protobuf ];
  libraries = with pkgs; [
    alsa-lib udev openssl ffmpeg_6-headless glib libdrm libgbm libGL
    libx11 libxext libxfixes libxdamage libxrandr libxcomposite
  ];
}
