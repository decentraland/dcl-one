/**
 * electron-builder accepts a single `beforePack` hook, so this composes the things that
 * have to happen before files are copied into the package.
 */
import copyNpmForAsar from './copy-npm-for-asar.js';
import downloadNode from './download-node.js';
import fetchDevtools from './fetch-devtools.js';
import provisionOxcBindings from './provision-oxc-bindings.js';

export default async function beforePack(context) {
  fetchDevtools();
  await copyNpmForAsar(context);
  await downloadNode(context);
  await provisionOxcBindings(context);
}
