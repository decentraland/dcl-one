# Validate the checked graph before using any manifest path to access the source tree.
{ root, manifest, filteredSource ? false }:
let
  inherit (builtins) all attrNames concatLists dirOf elem filter
    hashFile isAttrs isFloat isInt isList isString length listToAttrs map match readDir
    sort stringLength substring;
  web = "rendering/bevy-explorer/source/react-web/";
  manifestPath = web + "shared-inputs.json";
  domains = [ "library/ui" "library/shop" "library/catalyst-api" ];
  resolverNames = [ "package.json" "tsconfig.json" "jsconfig.json" ];
  requiredControls = map (name: web + name) [
    "package.json" "package-lock.json" "tsconfig.json" "vite.config.ts"
    "vitest.config.ts" "vite.shared.ts" "scripts/shared-inputs.mjs"
  ] ++ [ "library/ui/package.json" "library/ui/foundations/typography/font-source.json" ];
  less = a: b: builtins.lessThan a b;
  split = value: filter isString (builtins.split "/" value);
  prefix = start: value: stringLength value >= stringLength start
    && substring 0 (stringLength start) value == start;
  exact = keys: value: isAttrs value && attrNames value == sort less keys;
  sortedUnique = values: isList values && all isString values
    && values == attrNames (listToAttrs (map (name: { inherit name; value = true; }) values));
  safe = value: isString value && value != "" && !(prefix "/" value)
    && match ".*\\\\.*" value == null && match "[^[:cntrl:]]*" value != null
    && all (part: part != "" && part != "." && part != "..") (split value);
  reserved = name: elem name [ "node_modules" ".git" ".agent-work" ];
  outputName = name: prefix ".consumer-foundations-" name || prefix ".consumer-ssr-" name;
  ignoredEntry = name: kind: reserved name || (kind == "regular" && name == ".DS_Store") || (kind == "directory" && outputName name);
  domainFor = path: filter (domain: prefix (domain + "/") path) domains;
  firstParty = path: safe path && length (domainFor path) == 1
    && all (part: !(reserved part)) (split path)
    && all (part: !(outputName part)) (split (dirOf path));
  ownedRoot = path: prefix (web + "src/") path
    || elem path [ (web + "app.html") (web + "index.html") ];
  rootPaths = manifest.roots;
  rows = manifest.controls ++ manifest.files;
  rowNames = map (row: row.path) rows;
  controls = map (row: row.path) manifest.controls;
  files = map (row: row.path) manifest.files;
  safeInteger = value: (isInt value || isFloat value) && value >= 0
    && value <= 9007199254740991 && builtins.floor value == value;
  validLfs = row: !(row ? lfs) || (exact [ "oid" "size" ] row.lfs
    && row.lfs.oid == row.sha256 && safeInteger row.lfs.size);
  validRow = row: isAttrs row
    && (exact [ "path" "sha256" ] row || exact [ "path" "sha256" "lfs" ] row)
    && safe row.path && row.path != manifestPath
    && isString row.sha256 && match "[0-9a-f]{64}" row.sha256 != null
    && validLfs row;
  edgeLess = a: b: less a.from b.from || (a.from == b.from
    && (less a.to b.to || (a.to == b.to && less a.kind b.kind)));
  validEdge = edge: exact [ "from" "to" "kind" ] edge
    && safe edge.from && safe edge.to && isString edge.kind && edge.kind != ""
    && elem edge.from rowNames && elem edge.to rowNames;
  edgeKeys = map builtins.toJSON manifest.edges;
  validEntry = entry: exact [ "name" "kind" ] entry && safe entry.name
    && length (split entry.name) == 1 && elem entry.kind [ "regular" "directory" ]
    && !(ignoredEntry entry.name entry.kind);
  ancestors = stop: directory: [ directory ]
    ++ (if directory == stop then [] else ancestors stop (dirOf directory));
  expectedDirectories = attrNames (listToAttrs (map (name: { inherit name; value = true; })
    (concatLists (map (file: ancestors (builtins.head (domainFor file)) (dirOf file)) files))));
  validDirectory = row: exact [ "path" "entries" ] row && safe row.path
    && all (part: !(reserved part) && !(outputName part)) (split row.path)
    && isList row.entries && all validEntry row.entries
    && sortedUnique (map (entry: entry.name) row.entries);
  metadataValid = exact [ "schema" "controls" "roots" "files" "edges" "resolutionDirectories" ] manifest
    && (isInt manifest.schema || isFloat manifest.schema) && manifest.schema == 2
    && isList manifest.controls && isList manifest.files && isList manifest.edges
    && isList rootPaths && all safe rootPaths && all ownedRoot rootPaths && sortedUnique rootPaths
    && all validRow rows
    && sortedUnique controls && sortedUnique files
    && length (attrNames (listToAttrs (map (name: { inherit name; value = true; }) rowNames))) == length rows
    && all firstParty files && all (path: elem path controls) requiredControls
    && all (path: elem path rowNames) rootPaths
    && all validEdge manifest.edges && manifest.edges == sort edgeLess manifest.edges
    && length (attrNames (listToAttrs (map (name: { inherit name; value = true; }) edgeKeys))) == length edgeKeys
    && isList manifest.resolutionDirectories && all validDirectory manifest.resolutionDirectories
    && map (row: row.path) manifest.resolutionDirectories == expectedDirectories;

  fsPath = path: root + "/${path}";
  # readDir exposes the entry type without following a symlink. Every parent must be a directory.
  kindAt = path:
    let walk = parent: parts:
      let name = builtins.head parts; entries = readDir parent;
        kind = entries.${name} or "missing";
      in if length parts == 1 then kind
        else if kind == "directory" then walk (parent + "/${name}") (builtins.tail parts)
        else "missing";
    in walk root (split path);
  regular = path: kindAt path == "regular";
  validIdentity = row: regular row.path && hashFile "sha256" (fsPath row.path) == row.sha256;
  # The logical payload hash is checked first. No binary readFile or pointer acceptance is needed.
  # Python's mandatory verifier additionally checks the materialized LFS payload length.
  scanRoots = directory:
    let entries = readDir (fsPath directory);
    in concatLists (map (name:
      let path = directory + "/${name}"; kind = entries.${name};
      in if name == "node_modules" || path == web + "src/engine/generated" then []
        else if kind == "directory" then scanRoots path
        else if kind == "regular" then [ path ]
        else throw "Unsafe HUD root inventory entry") (attrNames entries));
  sourceRootKind = kindAt (web + "src");
  physicalRoots = sort less ((if sourceRootKind == "missing" then []
    else if sourceRootKind == "directory" then scanRoots (web + "src")
    else throw "Unsafe HUD source root") ++ concatLists (map (path:
      let kind = kindAt path;
      in if kind == "missing" then [] else if kind == "regular" then [ path ]
        else throw "Unsafe HUD HTML root") [ (web + "app.html") (web + "index.html") ]));
  resolverAncestors = file:
    let directory = dirOf file;
      paths = map (name: (if directory == "." then "" else directory + "/") + name) resolverNames;
    in filter (path: kindAt path != "missing") paths
      ++ (if directory == "." then [] else resolverAncestors directory);
  physicalNamespace = row:
    let entries = readDir (fsPath row.path);
      names = filter (name: !(ignoredEntry name entries.${name})) (attrNames entries);
    in map (name:
      let kind = entries.${name};
      in if safe name && elem kind [ "regular" "directory" ] then { inherit name kind; }
        else throw "Unsafe HUD resolution namespace entry") names;
  validNamespace = row: kindAt row.path == "directory"
    && (let actual = physicalNamespace row;
      in if filteredSource then all (entry: elem entry row.entries) actual else actual == row.entries);
  filesystemValid = all validIdentity rows && physicalRoots == rootPaths
    && all (path: elem path controls) (concatLists (map resolverAncestors rowNames))
    && all validNamespace manifest.resolutionDirectories;
in
  if metadataValid then filesystemValid else false
