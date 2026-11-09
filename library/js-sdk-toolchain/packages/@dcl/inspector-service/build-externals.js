const child_process = require('child_process');
const { builtinModules } = require('module');

const esmModulesToBundle = [
  '@dcl/sdk',
  '@dcl/ecs',
  '@dcl/smart-items',
  '@dcl-sdk/utils',
  '@dcl/gltf-validator-ts',
  '@dcl/ecs-math',
];

/** Modules left as runtime `require`s in the Node tooling bundle. */
function getNotBundledModules() {
  const child = child_process.execSync('pnpm ls --json --depth Infinity', {
    stdio: ['ignore', 'pipe', 'ignore'],
    maxBuffer: 64 * 1024 * 1024,
  });
  const [ret] = JSON.parse(child.toString());

  const externalModules = new Set();
  function traverseDependencies(obj) {
    for (const field of ['dependencies', 'devDependencies', 'optionalDependencies'])
      if (obj[field])
        for (let depName in obj[field]) {
          const dep = obj[field][depName];
          externalModules.add(depName);
          traverseDependencies(dep);
        }
  }
  traverseDependencies(ret);

  return Array.from(externalModules)
    .concat(builtinModules)
    .filter($ => !esmModulesToBundle.includes($));
}

module.exports = { getNotBundledModules };
