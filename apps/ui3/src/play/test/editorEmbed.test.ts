// The Creator Hub scene editor's viewport (lib/editorEmbed.ts): the editor's own scene is trusted,
// and only its launch takes the editor's session from the fragment.

import { describe, expect, it } from 'vitest'
import { EDITOR_SCENE, editorSession, isEditorScene } from '../lib/editorEmbed'
import { isTrustedSystemScene } from '../lib/systemScene'

const SESSION = '6f1c0e52-3c6e-4f53-9a43-2b7b9f8f6d10'

describe('the scene editor embed', () => {
  it('trusts the editor scene served next to /play, by path or absolute url', () => {
    expect(EDITOR_SCENE).toBe(new URL('/editor-scene/EditorScene', location.origin).href)
    for (const value of ['/editor-scene/EditorScene', '/editor-scene/EditorScene/', EDITOR_SCENE]) {
      expect(isEditorScene(value)).toBe(true)
      expect(isTrustedSystemScene(value)).toBe(true)
    }
    for (const value of ['/editor-scene/Other', 'https://evil.test/editor-scene/EditorScene', '/editor-scene']) {
      expect(isEditorScene(value)).toBe(false)
      expect(isTrustedSystemScene(value)).toBe(false)
    }
  })

  it('takes the session only for the editor scene and only a well-formed one', () => {
    expect(editorSession('/editor-scene/EditorScene', `#editorSession=${SESSION}`)).toBe(SESSION)
    expect(editorSession(null, `#editorSession=${SESSION}`)).toBeNull()
    expect(editorSession('http://localhost:8100', `#editorSession=${SESSION}`)).toBeNull()
    expect(editorSession('/editor-scene/EditorScene', '#editorSession=not-a-session')).toBeNull()
    expect(editorSession('/editor-scene/EditorScene', '')).toBeNull()
  })
})
