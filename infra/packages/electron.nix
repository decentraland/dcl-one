{ pkgs }:
let
  inherit (pkgs) lib;
  creatorHubPin = (lib.importJSON ../../apps/creator-hub/packages/creator-hub/package.json).devDependencies.electron;
  electron = (pkgs.callPackage "${pkgs.path}/pkgs/development/tools/electron/binary/generic.nix" { }) "45.3.1" {
    x86_64-linux = "c61d44206f8bd533d30310840b5baaa7fd0e8421cd711b8d1f719a7a336cbce7";
    aarch64-linux = "98bb5dbdcd5c6daaba6d2de293fe84bfae0ba61eb4288042a13723cf865b060a";
    aarch64-darwin = "ed40f8473a3c62a7c0463bd42f6724ccb8d5919cfdc64e0d9f65f61bdb3fe4d2";
    headers = "0lz8y3qf9yc7b0wrx2lg3mvldm7a0nsmra2x2qwk3qp1453wn37g";
  };
in
assert lib.assertMsg (electron.version == creatorHubPin)
  "infra/packages/electron.nix builds Electron ${electron.version}, but the Creator Hub pins ${creatorHubPin}";
electron
