// Where the Shop's avatar has its nametag (previewNametag, scene → page): the session keeps the engine's last word
// on it, and forgets it once the engine says there is none.
import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, waitFor } from '@testing-library/react'
import { renderSession } from '../harness'

afterEach(() => {
  cleanup()
})

describe("the Shop avatar's nametag", () => {
  it('stands where the engine last said, and is unknown once the engine has none', async () => {
    const h = renderSession({ userId: null })
    expect(h.session().shop.nametagAnchor ?? null).toBeNull()
    act(() => h.driver.emit({ kind: 'previewNametag', anchor: { x: 0.53, y: 0.12 } }))
    await waitFor(() => expect(h.session().shop.nametagAnchor).toEqual({ x: 0.53, y: 0.12 }))
    act(() => h.driver.emit({ kind: 'previewNametag', anchor: null }))
    await waitFor(() => expect(h.session().shop.nametagAnchor).toBeNull())
  })

  it("tells the engine how the Shop's page draws the avatar with its stage, and nothing more when not told", () => {
    const h = renderSession({ userId: null })
    const rect = { x: 0, y: -70, width: 1920, height: 1080 }
    act(() => h.session().backpack.stage(rect, 'names'))
    expect(h.driver.last('previewStage')).toEqual({ kind: 'previewStage', rect, framing: 'names' })
    act(() => h.session().backpack.stage(rect))
    expect(h.driver.last('previewStage')).toEqual({ kind: 'previewStage', rect })
    act(() => h.session().backpack.stage(null))
    expect(h.driver.last('previewStage')).toEqual({ kind: 'previewStage', rect: null })
  })
})
