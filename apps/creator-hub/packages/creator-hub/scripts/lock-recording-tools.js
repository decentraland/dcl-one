/** `pnpm run lock:recording-tools`, or `pnpm run check:recording-tools` for `--check`. */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  checkLock,
  npmCommand,
  readMinimumReleaseAge,
  writeCheckedLock,
} from './lock-optimizer-tools.js';

export const PLATFORMS = [
  ['darwin', 'arm64'],
  ['darwin', 'x64'],
  ['win32', 'x64'],
  ['win32', 'arm64'],
  ['linux', 'x64'],
  ['linux', 'arm64'],
];

const WHISPER_PKG = '@fugood/whisper.node';
const EXPECTED_INSTALL_SCRIPTS = [WHISPER_PKG];

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const toolsDir = path.join(scriptsDir, '../main/src/modules/recording/tools');
const WORKSPACE_FILE = path.join(scriptsDir, '../../../pnpm-workspace.yaml');

export const MANIFEST_FILE = path.join(toolsDir, 'recording-tools.package.json');

export function lockFile(platform, arch) {
  return path.join(toolsDir, `${platform}-${arch}.package-lock.json`);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

/** The package.json of one platform, as recording/tools.ts builds it for the app's install. */
export function platformManifest(base, platform, arch) {
  const version = base.dependencies[WHISPER_PKG];
  return {
    ...base,
    dependencies: {
      ...base.dependencies,
      [`@fugood/node-whisper-${platform}-${arch}`]: version,
    },
  };
}

/** The problems of every committed lock against the pinned package.json; none when empty. */
export function checkLocks(base, read = readJson) {
  const problems = [];
  for (const [platform, arch] of PLATFORMS) {
    const manifest = platformManifest(base, platform, arch);
    const lock = read(lockFile(platform, arch));
    problems.push(
      ...checkLock(manifest, lock, EXPECTED_INSTALL_SCRIPTS).map(
        problem => `${platform}-${arch}: ${problem}`,
      ),
    );
  }
  return problems;
}

/** Writes one platform's new lock, or throws and leaves the committed one as it is. */
export function writeCheckedPlatformLock(
  lockText,
  { manifest, platform, arch, lockFile: file = lockFile(platform, arch), ...options },
) {
  return writeCheckedLock(lockText, {
    ...options,
    manifest,
    lockFile: file,
    expectedInstallScripts: EXPECTED_INSTALL_SCRIPTS,
  });
}

function makeLock(manifest, platform, arch) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'recording-tools-'));
  const pastThisMachinesPlatformCheck = ['--force', `--os=${platform}`, `--cpu=${arch}`];
  try {
    fs.writeFileSync(path.join(tmp, 'package.json'), `${JSON.stringify(manifest, null, 2)}\n`);
    execFileSync(
      npmCommand(),
      [
        'install',
        '--package-lock-only',
        '--ignore-scripts',
        '--no-audit',
        '--no-fund',
        ...pastThisMachinesPlatformCheck,
      ],
      { cwd: tmp, stdio: 'inherit' },
    );
    return fs.readFileSync(path.join(tmp, 'package-lock.json'), 'utf8');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

function workspaceMinimumReleaseAge() {
  const minimumReleaseAge = readMinimumReleaseAge(fs.readFileSync(WORKSPACE_FILE, 'utf8'));
  if (minimumReleaseAge === undefined) {
    throw new Error(
      `${path.relative(process.cwd(), WORKSPACE_FILE)} sets no minimumReleaseAge. ` +
        'The release-age check of the lock reads it, so set it there first.',
    );
  }
  return minimumReleaseAge;
}

async function main() {
  const base = readJson(MANIFEST_FILE);
  if (process.argv.includes('--check')) {
    const problems = checkLocks(base);
    if (problems.length > 0) throw new Error(problems.join('\n'));
    console.log('every <platform>-<arch>.package-lock.json agrees with the pins');
    return;
  }
  const minimumReleaseAge = workspaceMinimumReleaseAge();
  for (const [platform, arch] of PLATFORMS) {
    const manifest = platformManifest(base, platform, arch);
    await writeCheckedPlatformLock(makeLock(manifest, platform, arch), {
      manifest,
      platform,
      arch,
      minimumReleaseAge,
    });
    console.log(`updated ${platform}-${arch}.package-lock.json`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
