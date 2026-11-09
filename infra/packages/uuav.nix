{ pkgs, craneLib }:
let
  inherit (pkgs) lib stdenv;
  isLinux = stdenv.hostPlatform.isLinux;
  isDarwin = stdenv.hostPlatform.isDarwin;
  root = ../..;
  uuav = root + /library/uuav;
  manifest = builtins.fromTOML (builtins.readFile (uuav + /Cargo.toml));
  toolchain = (builtins.fromTOML (builtins.readFile (uuav + /rust-toolchain.toml))).toolchain;
  windowsFfmpeg = builtins.fromJSON (builtins.readFile (root + /tooling/uuav/ffmpeg-windows.json));
  version = manifest.package.version;

  ffmpegSrc = pkgs.fetchFromGitHub {
    owner = "FFmpeg";
    repo = "FFmpeg";
    rev = "9047fa1b084f76b1b4d065af2d743df1b40dfb56";
    hash = "sha256-FdKhhCveEo5UodEoyUh3aBHABv3OT2VXmwBXE1ce3p0=";
  };
  mbedtls = pkgs.mbedtls.overrideAttrs (old: {
    cmakeFlags = old.cmakeFlags ++ [
      "-DUSE_SHARED_MBEDTLS_LIBRARY=OFF"
      "-DUSE_STATIC_MBEDTLS_LIBRARY=ON"
      "-DENABLE_PROGRAMS=OFF"
      "-DENABLE_TESTING=OFF"
    ];
    doCheck = false;
    postConfigure = "";
    postPatch = (old.postPatch or "") + ''
      perl scripts/config.pl set MBEDTLS_THREADING_C
      perl scripts/config.pl set MBEDTLS_THREADING_PTHREAD
    '';
  });
  libxml2 = (pkgs.libxml2.override { enableShared = false; }).overrideAttrs { doCheck = false; };
  libva = pkgs.libva-minimal;
  runtimeLibraries = [ "avcodec" "avdevice" "avfilter" "avformat" "avutil" "swresample" "swscale" ]
    ++ lib.optionals isLinux [ "va" "va-drm" ];
  systemIconv = ''
    install_name_tool -change ${lib.getLib pkgs.libiconv}/lib/libiconv.2.dylib /usr/lib/libiconv.2.dylib "$file"
    sign "$file"
    if otool -L "$file" | grep -qE '^[[:space:]]+/nix/store'; then
      echo "$file links the Nix store:"
      otool -L "$file"
      exit 1
    fi
  '';
  signHook = lib.optionals isDarwin [ pkgs.darwin.autoSignDarwinBinariesHook ];

  ffmpeg = stdenv.mkDerivation {
    pname = "uuav-ffmpeg";
    version = "8.1";
    src = ffmpegSrc;
    nativeBuildInputs = [ pkgs.nasm pkgs.pkg-config ] ++ signHook;
    buildInputs = [ pkgs.zlib libxml2 mbedtls ] ++ lib.optionals isLinux [ pkgs.libdrm libva ];
    configurePlatforms = [ ];
    configureFlags = [
      "--cc=${stdenv.cc.targetPrefix}cc"
      "--disable-autodetect"
      "--disable-doc"
      "--disable-programs"
      "--disable-static"
      "--enable-shared"
      "--enable-pthreads"
      "--enable-zlib"
      "--enable-libxml2"
      "--enable-mbedtls"
      "--enable-version3"
      "--pkg-config-flags=--static"
    ] ++ lib.optionals isLinux [ "--enable-libdrm" "--enable-vaapi" ]
    ++ lib.optionals isDarwin [ "--enable-videotoolbox" "--install-name-dir=@rpath" ];
    enableParallelBuilding = true;
    postInstall = ''
      mkdir -p $out/share/uuav
      cat LICENSE.md COPYING.LGPLv3 > $out/share/uuav/LICENSE-ffmpeg.txt
      cp ${mbedtls.src}/LICENSE $out/share/uuav/LICENSE-mbedtls.txt
    '' + lib.optionalString isLinux ''
      cp ${libva.src}/COPYING $out/share/uuav/LICENSE-libva.txt
      ln -s ${lib.getLib libva}/lib/libva.so.2 ${lib.getLib libva}/lib/libva-drm.so.2 $out/lib/
    '';
    postFixup = lib.optionalString isLinux ''
      for lib in $out/lib/lib*.so.*.*.*; do
        patchelf --set-rpath "\$ORIGIN:$(patchelf --print-rpath "$lib")" "$lib"
      done
    '' + lib.optionalString isDarwin ''
      for file in $out/lib/lib*.*.*.*.dylib; do
        ${systemIconv}
      done
    '';
  };

  src = lib.fileset.toSource {
    root = uuav;
    fileset = lib.fileset.unions [
      (uuav + /Cargo.toml)
      (uuav + /Cargo.lock)
      (uuav + /.cargo)
      (uuav + /src)
      (uuav + /uuav-ipc)
      (uuav + /uuav-server)
      (uuav + /uuav-client)
    ];
  };
  rust = pkgs.rust-bin.fromRustupToolchain {
    inherit (toolchain) channel components;
    profile = "minimal";
    targets = lib.optionals isLinux toolchain.targets;
  };
  crane = craneLib.overrideToolchain rust;
  common = {
    inherit src version;
    pname = "uuav";
    strictDeps = true;
    doCheck = false;
    nativeBuildInputs = [ pkgs.rustPlatform.bindgenHook ];
    buildInputs = [ ffmpeg ];
    FFMPEG_DIR = "${ffmpeg}";
    CARGO_TARGET_DIR = "target";
  } // lib.optionalAttrs isDarwin { DYLD_FALLBACK_LIBRARY_PATH = "${ffmpeg}/lib"; };
  cargoArtifacts = crane.buildDepsOnly common;
  helper = crane.buildPackage (common // {
    inherit cargoArtifacts;
    pname = "uuav-helper";
    cargoExtraArgs = "--locked -p uuav-server";
    meta.mainProgram = "uuav-helper";
  });
  smoke = crane.buildPackage (common // {
    inherit cargoArtifacts;
    pname = "uuav-smoke";
    cargoExtraArgs = "--locked -p uuav-client --example smoke";
    meta.mainProgram = "smoke";
  });
  checks = pkgs.linkFarm "uuav-checks" [
    { name = "fmt"; path = crane.cargoFmt { inherit src version; pname = "uuav"; }; }
    { name = "clippy"; path = crane.cargoClippy (common // {
        inherit cargoArtifacts;
        cargoClippyExtraArgs = "--workspace --all-targets -- -D warnings";
      }); }
    { name = "test"; path = crane.cargoTest (common // {
        inherit cargoArtifacts;
        cargoTestExtraArgs = "--workspace";
      }); }
  ];

  copyMajors = pattern: ''
    for lib in ${lib.concatStringsSep " " runtimeLibraries}; do
      for file in ${ffmpeg}/lib/lib$lib.*; do
        if [[ $(basename "$file") =~ ^lib$lib${pattern}$ ]]; then
          cp -L "$file" $out/
        fi
      done
      ls $out/lib$lib.* > /dev/null
    done
  '';
  dist = stdenv.mkDerivation {
    pname = "uuav-dist";
    inherit version;
    dontUnpack = true;
    dontPatchELF = true;
    dontStrip = true;
    nativeBuildInputs = signHook;
    passthru.target = if isDarwin then "macos-arm64" else "linux-${stdenv.hostPlatform.parsed.cpu.name}";
    installPhase = ''
      mkdir -p $out
      cp ${helper}/bin/uuav-helper $out/
      cp ${ffmpeg}/share/uuav/LICENSE-*.txt $out/
    '' + (if isLinux then copyMajors "\\.so\\.[0-9]+" + ''
      chmod -R u+w $out
      for file in $out/uuav-helper $out/lib*.so.*; do
        [[ $(patchelf --print-rpath "$file") == '$ORIGIN'* ]] || patchelf --set-rpath "\$ORIGIN:$(patchelf --print-rpath "$file")" "$file"
      done
    '' else copyMajors "\\.[0-9]+\\.dylib" + ''
      chmod -R u+w $out
      for file in $out/uuav-helper $out/*.dylib; do
        ${systemIconv}
      done
    '');
  };

  cross = pkgs.pkgsCross.mingwW64;
  windowsTarget = "x86_64-pc-windows-gnu";
  ffmpegWindows = stdenv.mkDerivation {
    pname = "uuav-ffmpeg-windows";
    version = windowsFfmpeg.asset;
    src = pkgs.fetchurl { inherit (windowsFfmpeg) url sha256; };
    nativeBuildInputs = [ pkgs.unzip ];
    dontFixup = true;
    installPhase = ''
      mkdir -p $out
      cp -R . $out/
    '';
  };
  windowsCommon = {
    inherit src version;
    pname = "uuav-windows";
    strictDeps = true;
    doCheck = false;
    nativeBuildInputs = [ cross.stdenv.cc ];
    CARGO_BUILD_TARGET = windowsTarget;
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-C link-arg=-Wl,--no-insert-timestamp -L native=${cross.windows.pthreads}/lib";
    FFMPEG_DIR = "${ffmpegWindows}";
    CARGO_TARGET_DIR = "target";
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    BINDGEN_EXTRA_CLANG_ARGS = "--target=x86_64-w64-mingw32 -isystem ${cross.windows.mingw_w64_headers}/include -isystem ${pkgs.llvmPackages.clang-unwrapped.lib}/lib/clang/${lib.versions.major pkgs.llvmPackages.clang-unwrapped.version}/include";
  };
  helperWindows = crane.buildPackage (windowsCommon // {
    cargoArtifacts = crane.buildDepsOnly windowsCommon;
    pname = "uuav-helper-windows";
    cargoExtraArgs = "--locked -p uuav-server";
  });
  distWindows = stdenv.mkDerivation {
    pname = "uuav-dist-windows";
    inherit version;
    dontUnpack = true;
    dontFixup = true;
    passthru.target = "windows-x86_64";
    installPhase = ''
      mkdir -p $out
      cp ${helperWindows}/bin/uuav-helper.exe $out/
      for lib in avcodec avformat avutil swresample swscale; do
        cp ${ffmpegWindows}/bin/$lib-*.dll $out/
      done
      cp ${ffmpegWindows}/LICENSE.txt $out/LICENSE-ffmpeg.txt
    '';
  };
in {
  uuav-ffmpeg = ffmpeg;
  uuav-helper = helper;
  uuav-smoke = smoke;
  uuav-checks = checks;
  uuav-dist = dist;
} // lib.optionalAttrs isLinux {
  uuav-ffmpeg-windows = ffmpegWindows;
  uuav-helper-windows = helperWindows;
  uuav-dist-windows = distWindows;
}
