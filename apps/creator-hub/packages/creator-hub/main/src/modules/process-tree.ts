import { execFile } from 'node:child_process';

type Done = (error?: Error | null) => void;

/**
 * Kills a process and all the processes below it. On Windows `taskkill` stops the tree by
 * force, with any signal. On the other systems each process of the tree gets the signal.
 * A process that is gone is not an error. `done` gets the error when the kill fails.
 */
export function killProcessTree(pid: number, signal?: NodeJS.Signals, done?: Done): void {
  if (process.platform === 'win32') {
    execFile('taskkill', ['/pid', String(pid), '/T', '/F'], { windowsHide: true }, error =>
      done?.(error),
    );
    return;
  }

  execFile('ps', ['-A', '-o', 'pid=', '-o', 'ppid='], (_error, stdout) => {
    try {
      for (const target of processTree(pid, parsePsOutput(stdout ?? ''))) {
        killProcess(target, signal);
      }
      done?.();
    } catch (error) {
      done?.(error as Error);
    }
  });
}

/** Reads the lines of `ps -o pid= -o ppid=` into the children of each parent. */
export function parsePsOutput(output: string): Map<number, number[]> {
  const children = new Map<number, number[]>();
  for (const line of output.split('\n')) {
    const match = /^\s*(\d+)\s+(\d+)\s*$/.exec(line);
    if (!match) continue;
    const [child, parent] = [Number(match[1]), Number(match[2])];
    children.set(parent, [...(children.get(parent) ?? []), child]);
  }
  return children;
}

/** Gives `root` and each process below it, one time each, with the children of a process before it. */
export function processTree(root: number, children: Map<number, number[]>): number[] {
  const tree = new Set<number>();
  const parents = [root];
  const seen = new Set(parents);
  for (const parent of parents) {
    for (const child of children.get(parent) ?? []) {
      tree.add(child);
      if (!seen.has(child)) {
        seen.add(child);
        parents.push(child);
      }
    }
    tree.add(parent);
  }
  return [...tree];
}

function killProcess(pid: number, signal?: NodeJS.Signals): void {
  try {
    process.kill(pid, signal);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
  }
}
