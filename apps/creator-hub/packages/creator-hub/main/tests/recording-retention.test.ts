import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ userData: '' }));
vi.mock('electron', () => ({ app: { getPath: () => mocks.userData } }));
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));

import {
  discard,
  getRecordingsDir,
  holdSession,
  pruneOldSessions,
  pruneProject,
  sweepLeftovers,
} from '../src/modules/recording/retention';

const DAY_MS = 24 * 60 * 60_000;

function session(project: string, name: string, ageMs = 0): string {
  const dir = path.join(getRecordingsDir(), project, name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, 'SESSION.md'), '#');
  const when = new Date(Date.now() - ageMs);
  fs.utimesSync(dir, when, when);
  return dir;
}

describe('when the app starts with old recordings on disk', () => {
  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-'));
  });

  afterEach(() => fs.rmSync(mocks.userData, { recursive: true, force: true }));

  it('should delete sessions older than 30 days and keep the newer ones', async () => {
    const old = session('scene-aaa', '2020-01-01_00-00-00.000-old', 31 * DAY_MS);
    const recent = session('scene-aaa', '2026-01-01_00-00-00.000-new', 29 * DAY_MS);
    await pruneOldSessions();
    expect(fs.existsSync(old)).toBe(false);
    expect(fs.existsSync(recent)).toBe(true);
  });

  it('should remove a project folder once nothing is left in it', async () => {
    const dir = session('scene-bbb', '2020-01-01_00-00-00.000-old', 31 * DAY_MS);
    fs.mkdirSync(path.join(getRecordingsDir(), 'scene-empty'), { recursive: true });
    await pruneOldSessions();
    expect(fs.existsSync(path.dirname(dir))).toBe(false);
    expect(fs.existsSync(path.join(getRecordingsDir(), 'scene-empty'))).toBe(false);
  });

  it('should leave a session the recorder is still using', async () => {
    const dir = session('scene-ccc', '2020-01-01_00-00-00.000-old', 31 * DAY_MS);
    const release = holdSession(dir, 'This recording is still running.');
    await pruneOldSessions();
    expect(fs.existsSync(dir)).toBe(true);
    release();
  });

  it('should keep a project folder that still holds a session', async () => {
    const dir = session('scene-fff', '2026-01-01_00-00-00.000-new');
    await pruneOldSessions();
    expect(fs.existsSync(path.dirname(dir))).toBe(true);
  });

  it('should delete nothing a project folder gained while it was listing', async () => {
    const projectDir = path.join(getRecordingsDir(), 'scene-ggg');
    fs.mkdirSync(projectDir, { recursive: true });
    const readdir = fsp.readdir as unknown as typeof fsp.readdir;
    const spy = vi.spyOn(fsp, 'readdir').mockImplementation(async (target, options) => {
      const entries = await readdir(target as string, options as never);
      if (String(target) === projectDir) session('scene-ggg', '2026-01-01_00-00-00.000-late');
      return entries as never;
    });
    await pruneOldSessions();
    spy.mockRestore();
    expect(fs.existsSync(path.join(projectDir, '2026-01-01_00-00-00.000-late'))).toBe(true);
  });

  it('should leave anything in the recordings folder that is not a project folder', async () => {
    const stray = path.join(getRecordingsDir(), 'notes.txt');
    fs.mkdirSync(getRecordingsDir(), { recursive: true });
    fs.writeFileSync(stray, 'keep me');
    const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-outside-'));
    fs.writeFileSync(path.join(outside, 'treasure.txt'), 'keep me too');
    fs.symlinkSync(outside, path.join(getRecordingsDir(), 'linked-project'));
    await pruneOldSessions();
    expect(fs.existsSync(stray)).toBe(true);
    expect(fs.existsSync(path.join(outside, 'treasure.txt'))).toBe(true);
    fs.rmSync(outside, { recursive: true, force: true });
  });

  it('should do nothing when there are no recordings at all', async () => {
    await expect(pruneOldSessions()).resolves.toBeUndefined();
  });
});

describe('when one project collects more sessions than the cap', () => {
  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-'));
  });

  afterEach(() => fs.rmSync(mocks.userData, { recursive: true, force: true }));

  it('should keep the newest 20 of them', async () => {
    for (let i = 0; i < 25; i++) {
      session('scene-ddd', `2026-01-01_00-00-${String(i).padStart(2, '0')}.000-x`);
    }
    const projectDir = path.join(getRecordingsDir(), 'scene-ddd');
    await pruneProject(projectDir);
    const left = fs.readdirSync(projectDir).sort();
    expect(left).toHaveLength(20);
    expect(left[0]).toBe('2026-01-01_00-00-05.000-x');
  });
});

describe('when a finished recording is discarded', () => {
  let dir: string;

  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-'));
    dir = session('scene-eee', '2026-01-01_00-00-00.000-x');
  });

  afterEach(() => fs.rmSync(mocks.userData, { recursive: true, force: true }));

  it('should delete only a session folder under the recordings dir', async () => {
    await expect(discard(getRecordingsDir())).rejects.toThrow('Not a session recording folder.');
    await expect(discard(path.dirname(dir))).rejects.toThrow('Not a session recording folder.');
    await expect(discard(path.join(dir, '..', '..', '..'))).rejects.toThrow(
      'Not a session recording folder.',
    );
    await discard(dir);
    expect(fs.existsSync(dir)).toBe(false);
  });

  it('should refuse a session the recorder is still using, with its own reason', async () => {
    const release = holdSession(dir, 'This recording is still being transcribed.');
    await expect(discard(dir)).rejects.toThrow('This recording is still being transcribed.');
    release();
    await discard(dir);
    expect(fs.existsSync(dir)).toBe(false);
  });
});

describe('when the app crashed in the middle of a transcription', () => {
  let dir: string;

  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'retention-'));
    dir = session('scene-hhh', '2026-01-01_00-00-00.000-x');
    for (const file of [
      'audio.wav',
      'transcribe-job.json',
      'transcribe-result.json',
      'transcribe-result.json.part',
    ]) {
      fs.writeFileSync(path.join(dir, file), 'left over');
    }
  });

  afterEach(() => fs.rmSync(mocks.userData, { recursive: true, force: true }));

  it('should delete what the recorder left behind and keep the session itself', async () => {
    await sweepLeftovers();
    expect(fs.readdirSync(dir)).toEqual(['SESSION.md']);
  });

  it('should leave a session the recorder is using right now', async () => {
    const release = holdSession(dir, 'This recording is still being transcribed.');
    await sweepLeftovers();
    release();
    expect(fs.readdirSync(dir).sort()).toEqual([
      'SESSION.md',
      'audio.wav',
      'transcribe-job.json',
      'transcribe-result.json',
      'transcribe-result.json.part',
    ]);
  });

  it('should look in every project folder and leave anything else alone', async () => {
    const other = session('scene-iii', '2026-01-01_00-00-00.000-y');
    fs.writeFileSync(path.join(other, 'audio.wav'), 'left over');
    fs.writeFileSync(path.join(other, 'events.jsonl'), '{}');
    fs.writeFileSync(path.join(getRecordingsDir(), 'notes.txt'), 'keep me');
    await sweepLeftovers();
    expect(fs.readdirSync(other).sort()).toEqual(['SESSION.md', 'events.jsonl']);
    expect(fs.existsSync(path.join(getRecordingsDir(), 'notes.txt'))).toBe(true);
  });
});
