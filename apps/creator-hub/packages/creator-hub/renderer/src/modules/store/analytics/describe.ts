import type { AppState } from '#store';

const MAX_ERROR_LENGTH = 200;
// Absolute paths name the creator's home folder, so they never reach analytics.
const ABSOLUTE_PATH = /(?:[A-Za-z]:)?[\\/](?:[^\s'"`:,;()[\]{}<>]+[\\/])+[^\s'"`:,;()[\]{}<>]*/g;

/** The error's message for an analytics property: file paths replaced, length capped. */
export function describeError(error: { message?: string; name?: string } | undefined): string {
  const message = error?.message || error?.name || 'unknown';
  return message.replace(ABSOLUTE_PATH, '<path>').slice(0, MAX_ERROR_LENGTH);
}

export function projectIdForPath(state: AppState, path: string | undefined): string | null {
  if (!path) return null;
  const project =
    state.workspace.projects.find($ => $.path === path) ??
    (state.editor.project?.path === path ? state.editor.project : undefined);
  return project?.id ?? null;
}
