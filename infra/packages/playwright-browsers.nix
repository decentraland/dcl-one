{ pkgs, consumer }:
let
  inherit (pkgs) lib;
  consumers = {
    creator-hub = { lockfile = ../../apps/creator-hub/pnpm-lock.yaml; package = "playwright"; };
    react-web = { lockfile = ../../rendering/bevy-explorer/source/react-web/pnpm-lock.yaml; package = "@playwright/test"; };
  };
  inherit (consumers.${consumer}) lockfile package;
  native = pkgs.playwright-driver.browsers;
  nativeRevision = name: pkgs.playwright-driver.browsersJSON.${name}.revision;
  revisionsByPlaywrightVersion = {
    "1.62" = { chromium = "1234"; ffmpeg = "1011"; };
  };
  importerKey = if lib.hasPrefix "@" package then "'${package}':" else "${package}:";
  lockLines = lib.splitString "\n" (builtins.readFile lockfile);
  importerAt = lib.lists.findFirstIndex (line: lib.trim line == importerKey) null lockLines;
  lockedVersion = lib.throwIf (importerAt == null) "${toString lockfile} locks no ${package}"
    (lib.head (lib.splitString "(" (lib.removePrefix "version: " (lib.trim (lib.elemAt lockLines (importerAt + 2))))));
  pinned = revisionsByPlaywrightVersion.${lib.versions.majorMinor lockedVersion} or (throw
    "infra/packages/playwright-browsers.nix names no browser revisions for ${package} ${lockedVersion} (${toString lockfile})");
in
pkgs.runCommand "playwright-browsers-${lockedVersion}" { } ''
  mkdir $out
  cp -P ${native}/* $out/
  ln -sfn ${native}/chromium-${nativeRevision "chromium"} $out/chromium-${pinned.chromium}
  ln -sfn ${native}/chromium_headless_shell-${nativeRevision "chromium-headless-shell"} $out/chromium_headless_shell-${pinned.chromium}
  ln -sfn ${native}/ffmpeg-${nativeRevision "ffmpeg"} $out/ffmpeg-${pinned.ffmpeg}
''
