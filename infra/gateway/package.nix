{ lib, stdenvNoCC, python3, nginx, makeWrapper }:
let
  table = builtins.toJSON ((import ./routes.nix) // {
    aliases = import ./root-aliases.nix;
    sites = lib.attrNames (lib.filterAttrs (name: type:
      type == "regular" && builtins.match ".*\\.(ts|tsx)" name != null
      && builtins.match ".*\\.(test|spec)\\.(ts|tsx)" name == null
    ) (builtins.readDir ../../apps/sites/src/routes/routes));
  });
in stdenvNoCC.mkDerivation {
  pname = "foundation-gateway";
  version = "0.1.0";
  dontUnpack = true;
  nativeBuildInputs = [ makeWrapper ];
  installPhase = ''
    mkdir -p "$out/bin" "$out/share/gateway"
    cp ${./gateway.py} "$out/share/gateway/gateway.py"
    cp ${builtins.toFile "gateway-routes.json" table} "$out/share/gateway/routes.json"
    makeWrapper ${python3}/bin/python3 "$out/bin/gateway" \
      --add-flags "$out/share/gateway/gateway.py" \
      --add-flags "--table $out/share/gateway/routes.json" \
      --add-flags "--nginx ${nginx}/bin/nginx" \
      --add-flags "--mime ${nginx}/conf/mime.types"
  '';
  meta.mainProgram = "gateway";
}
