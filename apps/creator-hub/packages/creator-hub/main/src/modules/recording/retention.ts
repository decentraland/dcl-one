import fsp from 'node:fs/promises';
import path from 'node:path';
import { app } from 'electron';
import log from 'electron-log/main';

import { RECORDING_RETENTION_DAYS } from '/shared/types/recording';

const KEEP_SESSIONS_PER_PROJECT = 20;
const MAX_SESSION_AGE_MS = RECORDING_RETENTION_DAYS * 24 * 60 * 60_000;
const LEFTOVER_FILES = [
  'audio.wav',
  'transcribe-job.json',
  'transcribe-result.json',
  'transcribe-result.json.part',
];

export function getRecordingsDir(): string {
  return path.join(app.getPath('userData'), 'recordings');
}

const inUse = new Map<string, string>();

/** Refuses {@link discard} and the age prune for this folder until the returned release runs. */
export function holdSession(dir: string, refusal: string): () => void {
  const target = path.resolve(dir);
  inUse.set(target, refusal);
  return () => {
    if (inUse.get(target) === refusal) inUse.delete(target);
  };
}

/** Keeps the newest sessions of one project, by folder name. */
export async function pruneProject(projectDir: string): Promise<void> {
  try {
    const entries = (await fsp.readdir(projectDir)).sort();
    const stale = entries.slice(0, Math.max(0, entries.length - KEEP_SESSIONS_PER_PROJECT));
    await Promise.all(stale.map(entry => remove(path.join(projectDir, entry))));
  } catch (error) {
    log.warn('[Recording] could not prune old sessions:', error);
  }
}

async function folders(dir: string): Promise<string[]> {
  const entries = await fsp.readdir(dir, { withFileTypes: true }).catch(() => []);
  return entries.filter(entry => entry.isDirectory()).map(entry => entry.name);
}

/** Drops every session the user has left for a month, and the project folders left empty. */
export async function pruneOldSessions(now = Date.now()): Promise<void> {
  try {
    const root = getRecordingsDir();
    for (const project of await folders(root)) {
      const projectDir = path.join(root, project);
      for (const entry of await folders(projectDir)) {
        const dir = path.join(projectDir, entry);
        const stat = await fsp.stat(dir).catch(() => null);
        if (!stat || now - stat.mtimeMs <= MAX_SESSION_AGE_MS) continue;
        await remove(dir);
      }
      await fsp.rmdir(projectDir).catch(() => undefined);
    }
  } catch (error) {
    log.warn('[Recording] could not prune the recordings folder:', error);
  }
}

/** Deletes what a crash or a force quit left in a session folder, audio above all. */
export async function sweepLeftovers(): Promise<void> {
  try {
    const root = getRecordingsDir();
    for (const project of await folders(root)) {
      const projectDir = path.join(root, project);
      for (const entry of await folders(projectDir)) {
        const dir = path.join(projectDir, entry);
        if (inUse.has(path.resolve(dir))) continue;
        for (const file of LEFTOVER_FILES) {
          await fsp.rm(path.join(dir, file), { force: true }).catch(() => undefined);
        }
      }
    }
  } catch (error) {
    log.warn('[Recording] could not sweep the recordings folder:', error);
  }
}

/** Deletes one finished session folder: the user discarded it before sending it. */
export async function discard(dir: string): Promise<void> {
  const target = sessionFolder(dir);
  const refusal = inUse.get(target);
  if (refusal) throw new Error(refusal);
  await fsp.rm(target, { recursive: true, force: true });
  log.info(`[Recording] discarded ${target}`);
}

/** The resolved path of a session folder (exactly two levels under recordings/), or a throw. */
export function sessionFolder(dir: string): string {
  const target = path.resolve(dir);
  const relative = path.relative(getRecordingsDir(), target);
  const inside = relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
  if (!inside || relative.split(path.sep).length !== 2) {
    throw new Error('Not a session recording folder.');
  }
  return target;
}

async function remove(dir: string): Promise<boolean> {
  if (inUse.has(path.resolve(dir))) return false;
  await fsp.rm(dir, { recursive: true, force: true });
  return true;
}
