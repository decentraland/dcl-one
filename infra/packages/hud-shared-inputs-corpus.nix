{ root, casesFile }:
let
  validate = import ./hud-shared-inputs.nix;
  cases = builtins.fromJSON (builtins.readFile casesFile);
  results = map (case:
    let value = validate {
        inherit root;
        manifest = case.manifest;
        filteredSource = case.filteredSource or false;
      };
      attempted = builtins.tryEval (builtins.deepSeq value value);
      accepted = attempted.success && attempted.value == true;
    in { inherit (case) name; inherit accepted; expected = case.nixValid or case.valid; endToEndExpected = case.valid; requiresPythonGuard = accepted && !case.valid; }) cases;
in
  assert builtins.isList cases;
  assert builtins.all (case: builtins.isAttrs case && builtins.isString case.name
    && builtins.isBool case.valid && case ? manifest
    && (!(case ? nixValid) || builtins.isBool case.nixValid)
    && (!(case ? filteredSource) || builtins.isBool case.filteredSource)) cases;
  assert builtins.all (result: result.accepted == result.expected) results;
  results
