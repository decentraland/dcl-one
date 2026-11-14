/**
 * Fetches the Chrome DevTools frontend the Hub opens against Unity's CDP server
 * (main/src/modules/devtools.ts) into `devtools-frontend/`.
 *
 * It used to be a git submodule. A submodule ties the build to being the root of its own git
 * checkout and to an SSH clone URL, so the pin lives here instead: a commit of the public
 * decentraland/devtools-frontend repo. Git verifies the objects against that hash, so the
 * commit id is the integrity check.
 *
 * Runs from `make install-creator-hub` and again from the electron-builder `beforePack` hook,
 * so packaging has it even when CI restored node_modules from cache and skipped the install.
 *
 * The performance panel's trace fixtures (~130 MB of test recordings) are left out; the
 * frontend never loads them.
 */
import path from 'path';
import fs from 'fs';
import os from 'os';
import { execFileSync } from 'child_process';
import { fileURLToPath } from 'url';

const REPO = 'https://github.com/decentraland/devtools-frontend.git';
const COMMIT = '6c29ec99b4136ffc2693a6645dde48c04d5cdf96';
const EXCLUDED = ['.git', path.join('panels', 'timeline', 'fixtures')];

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const target = path.join(packageDir, 'devtools-frontend');
const marker = path.join(target, '.pinned-commit');

function git(cwd, ...args) {
  execFileSync('git', args, { cwd, stdio: ['ignore', 'ignore', 'inherit'] });
}

function isCurrent() {
  try {
    return fs.readFileSync(marker, 'utf8').trim() === COMMIT;
  } catch {
    return false;
  }
}

export default function fetchDevtools() {
  if (isCurrent()) {
    console.log(`devtools-frontend is at ${COMMIT.slice(0, 12)}`);
    return;
  }
  const checkout = fs.mkdtempSync(path.join(os.tmpdir(), 'devtools-frontend-'));
  try {
    git(checkout, 'init', '--quiet');
    git(checkout, 'fetch', '--quiet', '--depth', '1', REPO, COMMIT);
    git(checkout, '-c', 'advice.detachedHead=false', 'checkout', '--quiet', COMMIT);

    fs.rmSync(target, { recursive: true, force: true });
    fs.cpSync(checkout, target, {
      recursive: true,
      filter: source => !EXCLUDED.includes(path.relative(checkout, source)),
    });
    fs.writeFileSync(marker, `${COMMIT}\n`);
    console.log(`devtools-frontend fetched at ${COMMIT.slice(0, 12)}`);
  } finally {
    fs.rmSync(checkout, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  fetchDevtools();
}
