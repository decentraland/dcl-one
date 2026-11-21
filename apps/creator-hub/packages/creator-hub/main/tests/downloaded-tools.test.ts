import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ run: vi.fn() }));
vi.mock('electron', () => ({ app: { getAppPath: () => '/app', getVersion: () => '1.0.0' } }));
vi.mock('../src/modules/bin', () => ({ run: mocks.run }));

import { installLocked } from '../src/modules/downloaded-tools';

describe('when a locked install times out', () => {
  let dir: string;

  beforeEach(() => {
    vi.clearAllMocks();
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'locked-install-'));
  });

  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  function stuckNpm(order: string[]) {
    let die!: () => void;
    mocks.run.mockImplementation(() => ({
      wait: () =>
        new Promise<Buffer>((_, reject) => {
          die = () => reject(new Error('killed'));
        }),
      kill: vi.fn(async () => {
        order.push('kill');
        await new Promise(resolve => setTimeout(resolve, 30));
        die();
        order.push('dead');
      }),
    }));
  }

  it('should kill npm and wait for it to die before failing', async () => {
    const order: string[] = [];
    stuckNpm(order);
    const outcome = installLocked({
      dir,
      packageJson: '{}',
      lockfile: '{}',
      expected: [],
      timeoutMs: 20,
      timeoutMessage: 'The tools install took too long.',
      what: 'The tools install',
    }).then(
      () => order.push('resolved'),
      (error: Error) => order.push(`rejected: ${error.message}`),
    );
    await outcome;
    expect(order).toEqual(['kill', 'dead', 'rejected: The tools install took too long.']);
  });

  it('should report what npm itself failed with when it is not a timeout', async () => {
    mocks.run.mockImplementation(() => ({
      wait: async () => {
        throw new Error('npm ERR! 403');
      },
      kill: vi.fn(),
    }));
    await expect(
      installLocked({
        dir,
        packageJson: '{}',
        lockfile: '{}',
        expected: [],
        timeoutMs: 1000,
        timeoutMessage: 'The tools install took too long.',
        what: 'The tools install',
      }),
    ).rejects.toThrow('npm ERR! 403');
  });
});

describe('when npm ci is interrupted after unpacking a package', () => {
  let dir: string;

  beforeEach(() => {
    vi.clearAllMocks();
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'locked-install-'));
  });

  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  function npmThatWrites(files: string[]) {
    mocks.run.mockImplementation(() => ({
      wait: async () => {
        for (const file of files) {
          fs.mkdirSync(path.join(dir, 'node_modules', path.dirname(file)), { recursive: true });
          fs.writeFileSync(path.join(dir, 'node_modules', file), '{}');
        }
        return Buffer.alloc(0);
      },
      kill: vi.fn(),
    }));
  }

  function install() {
    return installLocked({
      dir,
      packageJson: '{}',
      lockfile: '{}',
      expected: ['sharp'],
      timeoutMs: 1000,
      timeoutMessage: 'The tools install took too long.',
      what: 'The tools install',
    });
  }

  it('should refuse an install npm never finished, although the package is there', async () => {
    npmThatWrites(['sharp/package.json']);
    await expect(install()).rejects.toThrow('The tools install did not finish');
  });

  it('should accept the install once npm has written its own lockfile', async () => {
    npmThatWrites(['sharp/package.json', '.package-lock.json']);
    await expect(install()).resolves.toBeUndefined();
  });

  it('should not take the lockfile a previous install left for this one', async () => {
    fs.mkdirSync(path.join(dir, 'node_modules', 'sharp'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'node_modules', 'sharp', 'package.json'), '{}');
    fs.writeFileSync(path.join(dir, 'node_modules', '.package-lock.json'), '{"from":"before"}');
    npmThatWrites([]);
    await expect(install()).rejects.toThrow('The tools install did not finish');
  });
});
