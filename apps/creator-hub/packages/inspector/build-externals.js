const fs = require('fs');
const path = require('path');
const { builtinModules, createRequire } = require('module');

const esmModulesToBundle = [
  '@dcl/sdk',
  '@dcl/ecs',
  '@dcl/mini-rpc',
  '@dcl/asset-packs',
  '@dcl-sdk/utils',
  '@dcl/gltf-validator-ts',
  '@dcl/ecs-math',
  '@babylonjs/core',
];

/** The folder Node loads `name` from when `fromDir` requires it, or null when it is not installed. */
function findPackageDir(name, fromDir) {
  // Asking for the manifest keeps a package named like a Node builtin (`buffer`) on the same path.
  const manifest = `${name}/package.json`;
  const lookupDirs = createRequire(path.join(fromDir, 'package.json')).resolve.paths(manifest);
  for (const dir of lookupDirs || []) {
    if (fs.existsSync(path.join(dir, manifest))) return fs.realpathSync(path.join(dir, name));
  }
  return null;
}

/** Whether `dir` is a package of this repository; a dependency lives under a `node_modules`. */
function isLocalPackage(dir) {
  return !dir.split(path.sep).includes('node_modules');
}

/** Modules left as runtime `require`s in the Node tooling bundle: every dependency, direct or not. */
function getNotBundledModules() {
  const root = fs.realpathSync(__dirname);
  const externalModules = new Set();
  const visited = new Set();

  function traverseDependencies(dir) {
    if (visited.has(dir)) return;
    visited.add(dir);
    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    if (dir === root) externalModules.add(pkg.name);
    const names = Object.keys({
      ...pkg.dependencies,
      ...pkg.optionalDependencies,
      ...pkg.peerDependencies,
      ...(isLocalPackage(dir) ? pkg.devDependencies : {}),
    });
    for (const name of names) {
      externalModules.add(name);
      const depDir = findPackageDir(name, dir);
      if (depDir) traverseDependencies(depDir);
    }
  }

  traverseDependencies(root);

  return Array.from(externalModules)
    .concat(builtinModules)
    .filter($ => !esmModulesToBundle.includes($));
}

module.exports = { getNotBundledModules };
