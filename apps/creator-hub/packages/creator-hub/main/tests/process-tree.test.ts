import { execFile, spawn, type ChildProcess } from 'node:child_process';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';

import { killProcessTree, parsePsOutput, processTree } from '../src/modules/process-tree';

vi.mock('node:child_process', async importOriginal => {
  const actual = await importOriginal<{ execFile: typeof execFile }>();
  return { ...actual, execFile: vi.fn(actual.execFile) };
});

type ExecCallback = (error: Error | null, stdout: string, stderr: string) => void;

const PS_OUTPUT = [
  '    1     0',
  '  100     1',
  '  200   100',
  '  201   100',
  '  300   200',
  '  400     1',
  ' 5000   400',
  'garbage line',
  '',
].join('\n');

function isRunning(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

describe('parsePsOutput', () => {
  it('should give the children of each parent and skip the other lines', () => {
    expect(parsePsOutput(PS_OUTPUT)).toEqual(
      new Map([
        [0, [1]],
        [1, [100, 400]],
        [100, [200, 201]],
        [200, [300]],
        [400, [5000]],
      ]),
    );
  });
});

describe('processTree', () => {
  describe('when the process has children at more than one depth', () => {
    it('should give all of them and the process, and no other process', () => {
      expect(processTree(100, parsePsOutput(PS_OUTPUT)).sort()).toEqual([100, 200, 201, 300]);
    });
  });

  describe('when the process has no children', () => {
    it('should give the process only', () => {
      expect(processTree(300, parsePsOutput(PS_OUTPUT))).toEqual([300]);
    });
  });

  describe('when the list has a loop', () => {
    it('should give each process one time', () => {
      const children = new Map([
        [10, [11]],
        [11, [10]],
      ]);

      expect(processTree(10, children).sort()).toEqual([10, 11]);
    });
  });
});

describe('killProcessTree', () => {
  let platform: PropertyDescriptor | undefined;
  let kill: MockInstance<typeof process.kill>;
  let done: ReturnType<typeof vi.fn>;

  function setPlatform(value: NodeJS.Platform) {
    Object.defineProperty(process, 'platform', { value, configurable: true });
  }

  function mockExecFile(error: Error | null, stdout: string) {
    vi.mocked(execFile).mockImplementationOnce(((...args: unknown[]) => {
      (args.at(-1) as ExecCallback)(error, stdout, '');
    }) as unknown as typeof execFile);
  }

  beforeEach(() => {
    platform = Object.getOwnPropertyDescriptor(process, 'platform');
    kill = vi.spyOn(process, 'kill').mockReturnValue(true);
    done = vi.fn();
  });

  afterEach(() => {
    if (platform) Object.defineProperty(process, 'platform', platform);
    kill.mockRestore();
    vi.mocked(execFile).mockClear();
  });

  describe.each(['linux', 'darwin'] as const)('when the system is %s', system => {
    beforeEach(() => {
      setPlatform(system);
    });

    describe('and the process has children', () => {
      beforeEach(() => {
        mockExecFile(null, PS_OUTPUT);
        killProcessTree(100, 'SIGKILL', done);
      });

      it('should read the processes of the system one time', () => {
        expect(execFile).toHaveBeenCalledTimes(1);
        expect(execFile).toHaveBeenCalledWith(
          'ps',
          ['-A', '-o', 'pid=', '-o', 'ppid='],
          expect.any(Function),
        );
      });

      it('should send the signal to each process of the tree and to no other process', () => {
        expect(kill.mock.calls.map(([pid]) => pid).sort()).toEqual([100, 200, 201, 300]);
        expect(kill.mock.calls.every(([, signal]) => signal === 'SIGKILL')).toBe(true);
      });

      it('should end with no error', () => {
        expect(done).toHaveBeenCalledWith();
      });
    });

    describe('and a process of the tree is gone', () => {
      beforeEach(() => {
        mockExecFile(null, PS_OUTPUT);
        kill.mockImplementation(pid => {
          if (pid === 200) throw Object.assign(new Error('kill ESRCH'), { code: 'ESRCH' });
          return true;
        });
        killProcessTree(100, undefined, done);
      });

      it('should send the signal to the other processes and end with no error', () => {
        expect(kill.mock.calls.map(([pid]) => pid).sort()).toEqual([100, 200, 201, 300]);
        expect(done).toHaveBeenCalledWith();
      });
    });

    describe('and a process cannot be killed', () => {
      beforeEach(() => {
        mockExecFile(null, PS_OUTPUT);
        kill.mockImplementation(() => {
          throw Object.assign(new Error('kill EPERM'), { code: 'EPERM' });
        });
      });

      it('should give the error to the callback', () => {
        killProcessTree(100, 'SIGTERM', done);

        expect(done).toHaveBeenCalledWith(expect.objectContaining({ code: 'EPERM' }));
      });

      it('should not throw when there is no callback', () => {
        expect(() => killProcessTree(100)).not.toThrow();
      });
    });

    describe('and the list of processes cannot be read', () => {
      beforeEach(() => {
        mockExecFile(new Error('spawn ps ENOENT'), '');
        killProcessTree(100, 'SIGTERM', done);
      });

      it('should send the signal to the process itself', () => {
        expect(kill.mock.calls).toEqual([[100, 'SIGTERM']]);
        expect(done).toHaveBeenCalledWith();
      });
    });
  });

  describe('when the system is win32', () => {
    beforeEach(() => {
      setPlatform('win32');
    });

    describe('and taskkill succeeds', () => {
      beforeEach(() => {
        mockExecFile(null, 'SUCCESS');
        killProcessTree(4242, 'SIGKILL', done);
      });

      it('should stop the tree by force with taskkill', () => {
        expect(execFile).toHaveBeenCalledWith(
          'taskkill',
          ['/pid', '4242', '/T', '/F'],
          { windowsHide: true },
          expect.any(Function),
        );
        expect(kill).not.toHaveBeenCalled();
      });

      it('should end with no error', () => {
        expect(done).toHaveBeenCalledWith(null);
      });
    });

    describe('and taskkill fails', () => {
      it('should give the error to the callback', () => {
        const error = new Error('Access is denied.');
        mockExecFile(error, '');

        killProcessTree(4242, 'SIGKILL', done);

        expect(done).toHaveBeenCalledWith(error);
      });
    });
  });

  describe.skipIf(process.platform === 'win32')('when a real process has a child', () => {
    let parent: ChildProcess;
    let childPid: number;

    beforeEach(async () => {
      kill.mockRestore();
      const idle = 'setInterval(() => {}, 1000)';
      const script = `
        const child = require('node:child_process').spawn(process.execPath, ['-e', '${idle}']);
        console.log(child.pid);
        ${idle};
      `;
      parent = spawn(process.execPath, ['-e', script], { stdio: ['ignore', 'pipe', 'ignore'] });
      childPid = await new Promise<number>(resolve =>
        parent.stdout!.once('data', data => resolve(Number(String(data).trim()))),
      );
    });

    afterEach(() => {
      for (const pid of [parent.pid!, childPid]) {
        try {
          process.kill(pid, 'SIGKILL');
        } catch {
          // The test killed it.
        }
      }
    });

    it('should stop the process and its child', async () => {
      expect(isRunning(childPid)).toBe(true);
      const exit = new Promise(resolve => parent.once('exit', resolve));

      await new Promise<void>((resolve, reject) =>
        killProcessTree(parent.pid!, 'SIGKILL', error => (error ? reject(error) : resolve())),
      );

      await exit;
      await vi.waitFor(() => expect(isRunning(childPid)).toBe(false), { timeout: 10000 });
    });
  });
});
