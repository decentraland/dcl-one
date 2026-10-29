// The Creator Hub scene editor frames this app as its viewport: `?systemScene=` names the editor's
// own scene (served by Sites next to /play, apps/ui3/editor-scene) and the fragment carries the
// editor's session (`#editorSession=<uuid>`). Not part of upstream react-web.

import { PAGE_DIR } from './publicUrl'

// The static realm `pnpm run build` in apps/ui3/editor-scene exports into ui3/public/editor-scene.
export const EDITOR_SCENE = new URL('../editor-scene/EditorScene', PAGE_DIR).href

const SESSION = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

/** The editor scene, as a `?systemScene=` value names it (relative to this page or absolute). */
export function isEditorScene(value: string): boolean {
  try {
    return new URL(value.trim(), PAGE_DIR).href.replace(/\/+$/, '') === EDITOR_SCENE
  } catch {
    return false
  }
}

/** The editor session of an editor viewport, or null for any other launch. */
export function editorSession(systemScene: string | null, hash: string): string | null {
  if (systemScene == null || !isEditorScene(systemScene)) return null
  const session = new URLSearchParams(hash.replace(/^#/, '')).get('editorSession')
  return session != null && SESSION.test(session) ? session.toLowerCase() : null
}
