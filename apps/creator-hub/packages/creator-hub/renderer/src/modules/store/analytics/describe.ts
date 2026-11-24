import { redactText } from '/shared/redact';
import type { AppState } from '#store';

const MAX_ERROR_LENGTH = 200;

// Absolute paths name the creator's home folder and the folders under it (`Jane Doe`,
// `OneDrive - Acme Corp`), so none of a path reaches analytics. One pattern, so a URL's path is
// never read again as a file path:
//  1. a quoted path, taken whole, spaces included (Node's fs errors quote theirs);
//  2. a URL, which keeps its scheme and host and loses its path and query;
//  3. a bare path: a folder may hold spaces when a separator follows it, so the text between two
//     paths on one line can go with them; the last part stops at a space.
const PATH_OR_URL = new RegExp(
  [
    String.raw`(['"\x60])(?:[A-Za-z]:)?[\\/][^'"\x60\r\n]*\1`,
    String.raw`\b([a-z][a-z0-9+.-]{1,20}:\/\/[^\s/'"\x60?#]+)([^\s'"\x60]*)`,
    String.raw`(?:(?<![A-Za-z0-9])[A-Za-z]:)?[\\/](?:[^\\/\r\n'"\x60:;,()[\]{}]*[\\/])+[^\s\\/'"\x60:,;()[\]{}]*`,
  ].join('|'),
  'gi',
);

/**
 * The error's message for an analytics property: secrets, emails, URL paths and file paths
 * replaced, length capped.
 */
export function describeError(error: { message?: string; name?: string } | undefined): string {
  const message = error?.message || error?.name || 'unknown';
  return redactText(message)
    .text.replace(PATH_OR_URL, (match, quote?: string, origin?: string, rest?: string) => {
      if (quote) return `${quote}<path>${quote}`;
      if (origin) return rest ? `${origin}/<path>` : origin;
      return '<path>';
    })
    .slice(0, MAX_ERROR_LENGTH);
}

export function projectIdForPath(state: AppState, path: string | undefined): string | null {
  if (!path) return null;
  const project =
    state.workspace.projects.find($ => $.path === path) ??
    (state.editor.project?.path === path ? state.editor.project : undefined);
  return project?.id ?? null;
}
