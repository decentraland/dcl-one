const { builtinModules } = require('module');

// Optional native accelerators `ws` requires inside a try/catch; left as
// runtime requires so the bundle never carries platform binaries.
const optionalNative = ['bufferutil', 'utf-8-validate'];

/**
 * Modules left as runtime `require`s in the Node bundle: node's own and the optional natives.
 * Everything else, the SDK packages and every npm dependency, is bundled, so `dist/index.js`
 * loads with no `node_modules` beside it.
 */
function getNotBundledModules() {
  return builtinModules.concat(builtinModules.map($ => `node:${$}`)).concat(optionalNative);
}

module.exports = { getNotBundledModules };
