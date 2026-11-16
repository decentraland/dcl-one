import fs from 'node:fs/promises';
import path from 'node:path';
import { app } from 'electron';

import { run } from './bin';

// What the optimizer and the session recorder share for the toolchains they download on first
// use: a lockfile-pinned `npm ci` into a userData folder, and the worker bundle copied beside
// it so the bundled real Node can run it (a file inside the asar can't be executed, and the
// worker resolves its native dependencies by walking up from its own directory).

type LockedInstall = {
  dir: string;
  packageJson: string;
  lockfile: string;
  // Packages that must exist under node_modules afterwards, or the install counts as failed.
  expected: string[];
  timeoutMs: number;
  timeoutMessage: string;
  // What the error names, e.g. "The optimizer tools install".
  what: string;
  omitOptional?: boolean;
};

export async function exists(file: string): Promise<boolean> {
  try {
    await fs.access(file);
    return true;
  } catch {
    return false;
  }
}

// `npm ci` installs exactly the lockfile or fails; --ignore-scripts because downloaded code
// must not get to run on install (every package here ships prebuilt bindings).
export async function installLocked({
  dir,
  packageJson,
  lockfile,
  expected,
  timeoutMs,
  timeoutMessage,
  what,
  omitOptional = false,
}: LockedInstall): Promise<void> {
  await fs.writeFile(path.join(dir, 'package.json'), packageJson);
  await fs.writeFile(path.join(dir, 'package-lock.json'), lockfile);
  const install = run('npm', 'npm', {
    args: [
      'ci',
      ...(omitOptional ? ['--omit=optional'] : []),
      '--ignore-scripts',
      '--no-audit',
      '--no-fund',
      '--loglevel',
      'error',
    ],
    cwd: dir,
  });
  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      void install.kill();
      reject(new Error(timeoutMessage));
    }, timeoutMs);
  });
  try {
    await Promise.race([install.wait(), timeout]);
  } finally {
    clearTimeout(timer);
  }
  for (const pkg of expected) {
    if (!(await exists(path.join(dir, 'node_modules', pkg, 'package.json')))) {
      throw new Error(`${what} did not produce ${pkg}`);
    }
  }
}

// Copied on every run: `npm ci` wipes node_modules, and the app may have updated since.
export async function ensureWorkerPackage({
  toolsDir,
  pkg,
  bin,
  bundle,
}: {
  toolsDir: string;
  pkg: string;
  bin: string;
  // The bundle's file name under main/dist, e.g. "optimizer-worker.js".
  bundle: string;
}): Promise<void> {
  const source = path.join(app.getAppPath(), 'main', 'dist', bundle);
  const pkgDir = path.join(toolsDir, 'node_modules', ...pkg.split('/'));
  await fs.mkdir(pkgDir, { recursive: true });
  await fs.copyFile(source, path.join(pkgDir, 'index.js'));
  await fs.writeFile(
    path.join(pkgDir, 'package.json'),
    JSON.stringify(
      {
        name: pkg,
        version: app.getVersion(),
        private: true,
        type: 'module',
        bin: { [bin]: 'index.js' },
      },
      null,
      2,
    ),
  );
}
