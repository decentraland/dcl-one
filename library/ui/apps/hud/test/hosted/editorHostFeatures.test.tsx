// What the entry url's editor-features hands the editor. Its own file: the host is published once
// per page, so editorHost.test.tsx can only see the features of its first load.
import { describe, it, expect } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useEditorHost } from '../../features/editorHost/EditorHost'
import type { DclEditorHostV1 } from '@explorer-core/features/editorHost/host/host'
import { fakeSession } from '../harness'

describe('editor features', () => {
  it('passes advanced to the editor, and keeps experimental to the host', async () => {
    const { result } = renderHook(() => useEditorHost('?editor&editor-features=experimental,advanced', fakeSession()))
    await act(async () => void result.current!.load().catch(() => {}))
    const host = (window as Window & { __dclEditorHost?: DclEditorHostV1 }).__dclEditorHost
    expect(host?.features).toEqual({ advanced: true })
  })
})
