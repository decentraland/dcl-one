/**
 * Ensures the packaged app ships a loadable oxc-parser native binding for the arch being built.
 *
 * Why: oxc-parser is a napi-rs package whose native code lives in per-platform OPTIONAL
 * dependencies (@oxc-parser/binding-darwin-x64, -arm64, ...), each gated by os/cpu. The install
 * brings only the one matching the build host, so a `macos-latest` (arm64) runner never installs the
 * Intel binding. electron-builder then packs BOTH the arm64 and x64 dmgs from that single
 * node_modules, and the Intel dmg shipped with no binding at all — the main process crashed on
 * launch with "Failed to load native binding" (oxc-parser/bindings.js), once #1524 wired native
 * oxc-parser into the main process.
 *
 * beforePack runs once per target arch, so we fetch the matching darwin binding straight from its
 * npm tarball into node_modules. (An install would reconcile the whole workspace tree right
 * before packaging; `npm pack` touches only its own temp cwd.) The cpu:wasm32 fallback is left
 * out: it needs @napi-rs/wasm-runtime, which is not installed, so it could not load, and
 * electron-builder fails on a package whose own dependency is missing.
 *
 * Only needed on macOS: the Windows/Linux artifacts build on their own native runners, where the
 * install brings the correct binding on its own.
 *
 * Runs as part of the electron-builder `beforePack` hook, once per target arch.
 */
import path from 'path';
import fs from 'fs';
import os from 'os';
import { execFileSync } from 'child_process';

/**
 * electron-builder hands us an Arch enum value, whose numeric ordering is not something we want
 * to depend on. Prefer the name when it is available. (Mirrors download-node.js.)
 */
function archName(context) {
  const raw = context.arch;
  if (typeof raw === 'string') return raw;
  const names = { 0: 'ia32', 1: 'x64', 2: 'armv7l', 3: 'arm64', 4: 'universal' };
  return names[raw] ?? os.arch();
}

/**
 * Downloads a single npm package tarball and extracts it into destDir. `npm pack` writes only to
 * its own temp cwd, so this never reconciles or prunes the workspace's node_modules tree. No-op
 * when the sentinel file (proof the package is already present) already exists.
 */
function ensurePackage(spec, destDir, sentinel) {
  if (fs.existsSync(path.join(destDir, sentinel))) return false;

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'ch-oxc-'));
  try {
    execFileSync('npm', ['pack', spec], { cwd: tmp, stdio: ['ignore', 'ignore', 'inherit'] });
    const tgz = fs.readdirSync(tmp).find(f => f.endsWith('.tgz'));
    if (!tgz) throw new Error(`provision-oxc-bindings: npm pack produced no tarball for ${spec}`);
    fs.mkdirSync(destDir, { recursive: true });
    execFileSync('tar', ['-xzf', path.join(tmp, tgz), '-C', destDir, '--strip-components=1']);
    return true;
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

export default async function provisionOxcBindings(context) {
  const electronPlatform = context.electronPlatformName ?? process.platform;
  if (electronPlatform !== 'darwin' && electronPlatform !== 'mas') return;

  const arch = archName(context);
  if (arch !== 'x64' && arch !== 'arm64') {
    throw new Error(`provision-oxc-bindings: unexpected macOS arch ${arch}`);
  }

  const appDir = context.appDir ?? context.packager?.info?.appDir;
  if (!appDir) {
    console.warn('provision-oxc-bindings: no appDir in context');
    return;
  }
  const rootDir = path.resolve(appDir, '../..');
  const oxcDir = path.join(rootDir, 'node_modules/@oxc-parser');

  const version = JSON.parse(
    fs.readFileSync(path.join(rootDir, 'node_modules/oxc-parser/package.json'), 'utf8'),
  ).version;

  const nativePkg = `binding-darwin-${arch}`;
  const nativeFile = `parser.darwin-${arch}.node`;
  if (
    ensurePackage(`@oxc-parser/${nativePkg}@${version}`, path.join(oxcDir, nativePkg), nativeFile)
  ) {
    console.log(`provision-oxc-bindings: added @oxc-parser/${nativePkg}@${version}`);
  }

  // The native binding for this arch MUST be present or the packaged app crashes on launch.
  const nativePath = path.join(oxcDir, nativePkg, nativeFile);
  if (!fs.existsSync(nativePath)) {
    throw new Error(`provision-oxc-bindings: missing ${nativePath} for darwin-${arch}`);
  }
}
