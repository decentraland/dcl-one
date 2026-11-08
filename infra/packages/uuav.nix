{ pkgs, craneLib }:
let
  inherit (pkgs) lib stdenv;
  isLinux = stdenv.hostPlatform.isLinux;
  isDarwin = stdenv.hostPlatform.isDarwin;
  root = ../..;
  uuav = root + /library/uuav;
  manifest = builtins.fromTOML (builtins.readFile (uuav + /Cargo.toml));
  toolchain = (builtins.fromTOML (builtins.readFile (uuav + /rust-toolchain.toml))).toolchain;
  version = manifest.package.version;

  ffmpegSrc = pkgs.fetchFromGitHub {
    owner = "FFmpeg";
    repo = "FFmpeg";
    rev = "946fcce07b6dcd0331c8cc609192aeff5e1924f8";
    hash = "sha256-/c6cVfMSF6JJxCuJs58bxBlmBXDJ+jbnC4pELB97Qb0=";
  };
  ffmpegVersion = "9.0.2";
  ffmpegPatches = [ ./uuav-ffmpeg-hls-https.patch ];
  libva = pkgs.libva-minimal;
  staticDav1d = dav1d: (dav1d.override { withTools = false; withExamples = false; useVulkan = false; })
    .overrideAttrs (old: { mesonFlags = (old.mesonFlags or [ ]) ++ [ "-Ddefault_library=static" ]; });
  dav1d = staticDav1d pkgs.dav1d;
  runtimeLibraries = [ "avcodec" "avformat" "avutil" "swresample" "swscale" ]
    ++ lib.optionals isLinux [ "va" "va-drm" ];
  join = lib.concatStringsSep ",";
  videoDecoders = [ "h264" "hevc" "vp8" "vp9" "av1" "libdav1d" "mpeg4" "mpeg2video" "mjpeg" "png" "theora" ];
  audioDecoders = [ "aac" "aac_latm" "mp3" "mp3float" "opus" "vorbis" "flac" "ac3" "eac3"
    "pcm_s16le" "pcm_s16be" "pcm_s24le" "pcm_f32le" "pcm_alaw" "pcm_mulaw" "pcm_u8" ];
  demuxers = [ "mov" "matroska" "hls" "mpegts" "mpegps" "flv" "ogg" "wav" "mp3" "aac" "flac" "ac3" "image2" "rtsp" "sdp" ];
  parsers = [ "h264" "hevc" "vp8" "vp9" "av1" "mpeg4video" "mpegvideo" "mjpeg" "png"
    "aac" "aac_latm" "mpegaudio" "opus" "vorbis" "flac" "ac3" ];
  protocols = [ "tcp" "udp" "rtp" "file" "data" ];
  amxHeaderGuards = [
    "__AMXINTRIN_H" "__AMX_FP16INTRIN_H" "__AMX_COMPLEXINTRIN_H" "__AMXFP8INTRIN_H"
    "__AMX_TRANSPOSEINTRIN_H" "__AMXMOVRSINTRIN_H" "__AMX_MOVRS_TRANSPOSEINTRIN_H"
    "__AMX_AVX512INTRIN_H" "__AMX_TF32INTRIN_H" "__AMX_TF32TRANSPOSEINTRIN_H"
    "__AMX_BF16TRANSPOSEINTRIN_H" "__AMX_FP16TRANSPOSEINTRIN_H" "__AMX_COMPLEXTRANSPOSEINTRIN_H" ];
  bindgenSkipsAmx = lib.concatMapStringsSep " " (guard: "-D${guard}") amxHeaderGuards;
  bsfs = [ "extract_extradata" "h264_mp4toannexb" "hevc_mp4toannexb" "aac_adtstoasc"
    "vp9_superframe_split" "av1_frame_split" "mpeg4_unpack_bframes" ];
  hwaccels = {
    vaapi = [ "h264" "hevc" "vp8" "vp9" "av1" "mpeg2" "mpeg4" "mjpeg" ];
    videotoolbox = [ "h264" "hevc" "vp9" "av1" "mpeg2" "mpeg4" ];
    d3d11va = [ "h264" "hevc" "vp9" "av1" "mpeg2" ];
    d3d11va2 = [ "h264" "hevc" "vp9" "av1" "mpeg2" ];
  };
  componentFlags = apis: [
    "--disable-everything"
    "--disable-avdevice"
    "--disable-avfilter"
    "--enable-decoder=${join (videoDecoders ++ audioDecoders)}"
    "--enable-demuxer=${join demuxers}"
    "--enable-parser=${join parsers}"
    "--enable-protocol=${join protocols}"
    "--enable-bsf=${join bsfs}"
    "--enable-hwaccel=${join (lib.concatMap (api: map (codec: "${codec}_${api}") hwaccels.${api}) apis)}"
  ];
  commonFlags = [
    "--disable-autodetect"
    "--disable-doc"
    "--disable-programs"
    "--disable-static"
    "--enable-shared"
    "--enable-zlib"
    "--enable-libdav1d"
    "--pkg-config-flags=--static"
  ];
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
    version = ffmpegVersion;
    src = ffmpegSrc;
    patches = ffmpegPatches;
    nativeBuildInputs = [ pkgs.nasm pkgs.pkg-config ] ++ signHook;
    buildInputs = [ pkgs.zlib dav1d ] ++ lib.optionals isLinux [ pkgs.libdrm libva ];
    configurePlatforms = [ ];
    configureFlags = [
      "--cc=${stdenv.cc.targetPrefix}cc"
      "--enable-pthreads"
    ] ++ commonFlags
    ++ lib.optionals isLinux ([ "--enable-libdrm" "--enable-vaapi" ] ++ componentFlags [ "vaapi" ])
    ++ lib.optionals isDarwin ([ "--enable-videotoolbox" "--install-name-dir=@rpath" ] ++ componentFlags [ "videotoolbox" ]);
    enableParallelBuilding = true;
    postInstall = ''
      mkdir -p $out/share/uuav
      cat LICENSE.md COPYING.LGPLv2.1 > $out/share/uuav/LICENSE-ffmpeg.txt
      cp ${dav1d.src}/COPYING $out/share/uuav/LICENSE-dav1d.txt
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
        doCheck = true;
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
  crossPrefix = cross.stdenv.cc.targetPrefix;
  crossDav1d = staticDav1d cross.dav1d;
  windowsTarget = "x86_64-pc-windows-gnu";
  ffmpegWindows = cross.stdenv.mkDerivation {
    pname = "uuav-ffmpeg-windows";
    version = ffmpegVersion;
    src = ffmpegSrc;
    patches = ffmpegPatches;
    depsBuildBuild = [ cross.buildPackages.stdenv.cc ];
    nativeBuildInputs = [ cross.buildPackages.nasm cross.buildPackages.pkg-config ];
    buildInputs = [ (cross.zlib.override { shared = false; }) crossDav1d ];
    configurePlatforms = [ ];
    configureFlags = [
      "--enable-cross-compile"
      "--target-os=mingw32"
      "--arch=x86_64"
      "--cross-prefix=${crossPrefix}"
      "--cc=${crossPrefix}cc"
      "--host-cc=cc"
      "--pkg-config=${crossPrefix}pkg-config"
      "--extra-ldflags=-static-libgcc"
      "--extra-ldflags=-Wl,--no-insert-timestamp"
      "--disable-pthreads"
      "--enable-w32threads"
      "--enable-d3d11va"
    ] ++ commonFlags ++ componentFlags [ "d3d11va" "d3d11va2" ];
    enableParallelBuilding = true;
    dontFixup = true;
    postInstall = ''
      for archive in $out/lib/*.dll.a; do
        ${crossPrefix}ranlib -D "$archive"
      done
      mkdir -p $out/share/uuav
      cat LICENSE.md COPYING.LGPLv2.1 > $out/share/uuav/LICENSE-ffmpeg.txt
      cp ${crossDav1d.src}/COPYING $out/share/uuav/LICENSE-dav1d.txt
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
    CC_x86_64_pc_windows_gnu = "${crossPrefix}cc";
    AR_x86_64_pc_windows_gnu = "${crossPrefix}ar";
    FFMPEG_DIR = "${ffmpegWindows}";
    CARGO_TARGET_DIR = "target";
    LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    BINDGEN_EXTRA_CLANG_ARGS = "--target=x86_64-w64-mingw32 -isystem ${cross.windows.mingw_w64_headers}/include -isystem ${pkgs.llvmPackages.clang-unwrapped.lib}/lib/clang/${lib.versions.major pkgs.llvmPackages.clang-unwrapped.version}/include ${bindgenSkipsAmx}";
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
      cp ${ffmpegWindows}/share/uuav/LICENSE-*.txt $out/
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
