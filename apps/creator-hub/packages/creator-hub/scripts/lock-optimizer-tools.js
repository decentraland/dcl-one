/**
 * Regenerates, or checks, the optimizer toolchain lockfile of its pinned package.json:
 *   main/src/modules/optimizer/tools/optimizer-tools.package.json      (edit versions here)
 *   main/src/modules/optimizer/tools/optimizer-tools.package-lock.json (generated, commit it)
 *
 * The lockfile is what the app installs with `npm ci` on first use (see tools.ts), so every
 * package the optimizer downloads is pinned to an exact registry tarball + integrity hash.
 * `npm ci` downloads whatever `resolved` names and never compares it with the entry's name, so
 * the check here is what binds each entry to the registry tarball of its own name and version.
 * An entry with a `name` that is not the name of its key (an npm alias) is refused: the lock itself
 * cannot show that the alias is intended.
 *
 * `pnpm run lock:optimizer-tools`, after bumping a version, writes the new lockfile only when it
 * passes the checks of `--check`, every integrity in it is the one the registry publishes for
 * that release, and every release in it is older than the workspace's `minimumReleaseAge`
 * (pnpm-workspace.yaml). pnpm holds the workspace's own installs to that age; the app installs
 * exactly this lockfile, so here the age is checked when the lock is made.
 *
 * `pnpm run check:optimizer-tools` (`--check`) reads the committed files only, with no network.
 */
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REGISTRY = 'https://registry.npmjs.org/';
const NAME = '(?:@[A-Za-z0-9~-][A-Za-z0-9._~-]*/)?[A-Za-z0-9~-][A-Za-z0-9._~-]*';
const PACKAGE_NAME = new RegExp(`^${NAME}$`);
const ENTRY_KEY = new RegExp(`^(?:node_modules/${NAME}/)*node_modules/(${NAME})$`);
// No `/` in a version: `resolved` is compared as text, and a path segment could reach another file.
const VERSION = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
// Exactly one hash: with several, npm accepts a tarball that matches any of them.
const SHA512 = /^sha512-[A-Za-z0-9+/]{86}==$/;
const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const toolsDir = path.join(scriptsDir, '../main/src/modules/optimizer/tools');
const WORKSPACE_FILE = path.join(scriptsDir, '../../../pnpm-workspace.yaml');

export const MANIFEST_FILE = path.join(toolsDir, 'optimizer-tools.package.json');
export const LOCK_FILE = path.join(toolsDir, 'optimizer-tools.package-lock.json');

// The app installs with --ignore-scripts, so a package that needs its install script arrives
// broken. Add a name here only once that package is known to work without the script.
const EXPECTED_INSTALL_SCRIPTS = [];

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

/** Each entry with its key, the package name its key gives (`keyName`) and its `name`. */
function lockEntries(lock) {
  return Object.entries(lock.packages ?? {})
    .filter(([key]) => key !== '')
    .map(([key, entry]) => {
      const keyName = ENTRY_KEY.exec(key)?.[1];
      return { ...entry, key, keyName, name: entry.name ?? keyName };
    });
}

/** The one registry tarball of a release; undefined when the name or version is not well formed. */
export function registryTarball(name, version) {
  if (typeof name !== 'string' || typeof version !== 'string') return undefined;
  if (!PACKAGE_NAME.test(name) || !VERSION.test(version)) return undefined;
  return `${REGISTRY}${name}/-/${name.slice(name.indexOf('/') + 1)}-${version}.tgz`;
}

/** The problems of a lock against the pinned package.json; none when the list is empty. */
export function checkLock(manifest, lock, expectedInstallScripts = EXPECTED_INSTALL_SCRIPTS) {
  const problems = [];
  const pins = manifest.dependencies ?? {};
  const declared = lock.packages?.['']?.dependencies ?? {};
  for (const name of new Set([...Object.keys(pins), ...Object.keys(declared)])) {
    const version = lock.packages?.[`node_modules/${name}`]?.version;
    if (declared[name] !== pins[name] || version !== pins[name]) {
      problems.push(
        `${name}: pinned ${pins[name]}, the lock declares ${declared[name]} and has ${version}`,
      );
    }
  }
  for (const entry of lockEntries(lock)) {
    const tarball = registryTarball(entry.name, entry.version);
    if (entry.keyName === undefined) {
      problems.push(`${entry.key}: the key is not a node_modules path of package names`);
    } else if (entry.name !== entry.keyName) {
      problems.push(`${entry.key}: is named ${entry.name}, not ${entry.keyName}`);
    }
    if (tarball === undefined) {
      problems.push(`${entry.key}: ${entry.name}@${entry.version} is not a name and a version`);
    } else if (entry.resolved !== tarball) {
      problems.push(`${entry.key}: resolves to ${entry.resolved}, not to ${tarball}`);
    }
    if (!entry.integrity) problems.push(`${entry.key}: no integrity`);
    else if (!SHA512.test(entry.integrity)) {
      problems.push(`${entry.key}: the integrity is not one sha512 hash (${entry.integrity})`);
    }
    if (entry.hasInstallScript && !expectedInstallScripts.includes(entry.name)) {
      problems.push(`${entry.key}: has an install script, which is not expected`);
    }
  }
  return problems;
}

/** Minutes, as pnpm reads the setting; undefined when the file does not set it. */
export function readMinimumReleaseAge(workspaceYaml) {
  const match = /^minimumReleaseAge:[ \t]*(\d+)[ \t]*(?:#.*)?$/m.exec(workspaceYaml);
  return match ? Number(match[1]) : undefined;
}

/**
 * The registry's `time` and `versions` maps of each package. A request carries the package name
 * and no more.
 */
export async function fetchRegistryPackages(names, fetchImpl = fetch) {
  const packages = new Map();
  await Promise.all(
    [...new Set(names)].map(async name => {
      const response = await fetchImpl(REGISTRY + name.replace('/', '%2F'));
      if (!response.ok) throw new Error(`${name}: the registry answered ${response.status}`);
      const { time = {}, versions = {} } = await response.json();
      packages.set(name, { time, versions });
    }),
  );
  return packages;
}

/** The entries of a lock whose integrity is not the one the registry publishes for the release. */
export function registryIntegrityProblems(lock, packages) {
  const problems = [];
  for (const { name, version, integrity } of lockEntries(lock)) {
    const published = packages.get(name)?.versions[version]?.dist?.integrity;
    if (integrity !== published) {
      problems.push(
        `${name}@${version}: the lock has integrity ${integrity}, the registry has ${published}`,
      );
    }
  }
  return problems;
}

/** The entries of a lock that are not yet `minimumReleaseAge` minutes old, or have no date. */
export function releaseAgeProblems(lock, packages, minimumReleaseAge, now = Date.now()) {
  const problems = [];
  for (const { name, version } of lockEntries(lock)) {
    const published = Date.parse(packages.get(name)?.time[version]);
    if (Number.isNaN(published)) {
      problems.push(`${name}@${version}: the registry gives no publish time`);
    } else if (now - published < minimumReleaseAge * 60_000) {
      problems.push(
        `${name}@${version}: published ${new Date(published).toISOString()}, ` +
          `less than ${minimumReleaseAge} minutes ago`,
      );
    }
  }
  return problems;
}

/** Writes a new lock, or throws and leaves the committed one as it is. */
export async function writeCheckedLock(
  lockText,
  {
    manifestFile = MANIFEST_FILE,
    lockFile = LOCK_FILE,
    minimumReleaseAge,
    fetchImpl = fetch,
    now = Date.now(),
  },
) {
  const lock = JSON.parse(lockText);
  const problems = checkLock(readJson(manifestFile), lock);
  if (problems.length === 0) {
    const names = lockEntries(lock).map(entry => entry.name);
    const packages = await fetchRegistryPackages(names, fetchImpl);
    problems.push(...registryIntegrityProblems(lock, packages));
    problems.push(...releaseAgeProblems(lock, packages, minimumReleaseAge, now));
  }
  if (problems.length > 0) {
    throw new Error(`The new lock was not written:\n${problems.join('\n')}`);
  }
  fs.writeFileSync(lockFile, lockText);
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

function makeLock() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'optimizer-tools-'));
  try {
    fs.copyFileSync(MANIFEST_FILE, path.join(tmp, 'package.json'));
    execFileSync(
      'npm',
      ['install', '--package-lock-only', '--ignore-scripts', '--no-audit', '--no-fund'],
      { cwd: tmp, stdio: 'inherit' },
    );
    return fs.readFileSync(path.join(tmp, 'package-lock.json'), 'utf8');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

async function main() {
  if (process.argv.includes('--check')) {
    const problems = checkLock(readJson(MANIFEST_FILE), readJson(LOCK_FILE));
    if (problems.length > 0) throw new Error(problems.join('\n'));
    console.log('optimizer-tools.package-lock.json agrees with the pins');
    return;
  }
  const minimumReleaseAge = workspaceMinimumReleaseAge();
  await writeCheckedLock(makeLock(), { minimumReleaseAge });
  console.log('updated optimizer-tools.package-lock.json');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
