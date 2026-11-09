# Tool environments the Bevy Explorer workflow takes from the flake on Linux
# (.github/actions/bevy-explorer-tools delivers them from the Actions cache).
{ pkgs }: {
  bevy-tools = pkgs.buildEnv {
    name = "bevy-explorer-tools";
    paths = [ pkgs.protobuf pkgs.taplo pkgs.wasm-pack ];
  };
}
