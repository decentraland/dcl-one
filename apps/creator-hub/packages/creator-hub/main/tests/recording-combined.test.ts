import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ userData: '' }));
vi.mock('electron', () => ({ app: { getPath: () => mocks.userData } }));
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));

import { combinedContext } from '../src/modules/recording/combined';
import { getRecordingsDir } from '../src/modules/recording/retention';

const T0 = Date.UTC(2026, 10, 18, 12, 0, 0);

function session(name: string, startedAt: number, events: number): string {
  const dir = path.join(getRecordingsDir(), 'scene-abc', name);
  fs.mkdirSync(dir, { recursive: true });
  const lines = Array.from({ length: events }, (_, i) =>
    JSON.stringify({
      t: startedAt + 1000 + i * 100,
      kind: 'select',
      text: `Selected "Crate_${i}" (#${600 + i})`,
      entity: { id: 600 + i, name: `Crate_${i}` },
    }),
  );
  fs.writeFileSync(path.join(dir, 'events.jsonl'), `${lines.join('\n')}\n`);
  fs.writeFileSync(
    path.join(dir, 'transcript.json'),
    JSON.stringify({
      startedAt,
      endedAt: startedAt + 60_000,
      audioStartedAt: startedAt,
      words: [
        { start: 500, end: 700, text: ' delete', segment: 0 },
        { start: 700, end: 900, text: ' this', segment: 0 },
      ],
    }),
  );
  fs.writeFileSync(path.join(dir, 'SESSION.md'), '#');
  return dir;
}

describe('combinedContext', () => {
  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'recording-combined-'));
  });

  afterEach(() => {
    fs.rmSync(mocks.userData, { recursive: true, force: true });
  });

  describe('when two recordings go out together', () => {
    let context: string;
    let dirs: string[];

    beforeEach(async () => {
      dirs = [session('a', T0, 5), session('b', T0 + 120_000, 5)];
      context = await combinedContext(dirs);
    });

    it('should number the blocks in recording order, each linking its SESSION.md', () => {
      const first = context.indexOf('Recording 1 of 2');
      const second = context.indexOf('Recording 2 of 2');
      expect(first).toBeGreaterThan(-1);
      expect(second).toBeGreaterThan(first);
      expect(context.indexOf(path.join(dirs[1], 'SESSION.md'))).toBeGreaterThan(second);
    });

    it('should say once that only the SAID lines are the user words', () => {
      expect(context.match(/Only the SAID lines are the user's words/g)).toHaveLength(1);
    });
  });

  describe('when five long recordings go out together', () => {
    let context: string;

    beforeEach(async () => {
      const dirs = [0, 1, 2, 3, 4].map(i => session(`s${i}`, T0 + i * 700_000, 400));
      context = await combinedContext(dirs);
    });

    it('should stay within the one context budget', () => {
      expect(context.length).toBeLessThanOrEqual(6000);
    });

    it('should still give every recording its block', () => {
      expect(context).toContain('Recording 5 of 5');
    });
  });

  describe('when a folder is not a session recording', () => {
    it('should refuse it', async () => {
      await expect(combinedContext([path.join(mocks.userData, 'elsewhere')])).rejects.toThrow(
        'Not a session recording folder.',
      );
    });
  });
});
