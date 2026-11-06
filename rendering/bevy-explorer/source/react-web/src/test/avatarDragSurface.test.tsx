// The page's hold on the preview avatar: a drag turns it, the wheel zooms its camera within its bounds, and
// unmounting gives the zoom back.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, waitFor } from '@testing-library/react'
import { AvatarDragSurface, DEGREES_PER_PX } from '../features/engine/AvatarDragSurface'
import { PREVIEW_MAX_ZOOM } from '../engine/protocol'

function surface() {
  const onRotate = vi.fn()
  const onZoom = vi.fn()
  const view = render(<AvatarDragSurface onRotate={onRotate} onZoom={onZoom} />)
  const el = view.container.firstElementChild as HTMLElement
  return { el, onRotate, onZoom, view }
}

const frame = (): Promise<void> => new Promise((r) => setTimeout(r, 40))

describe('AvatarDragSurface', () => {
  it('turns the avatar by the distance dragged, a frame at a time', async () => {
    const { el, onRotate } = surface()
    fireEvent.pointerDown(el, { pointerId: 1, button: 0, clientX: 100 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 110 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 140 })
    await waitFor(() => expect(onRotate).toHaveBeenCalled())
    // Both moves land in one frame: 40px, the way the pointer went.
    expect(onRotate).toHaveBeenCalledTimes(1)
    expect(onRotate).toHaveBeenLastCalledWith(40 * DEGREES_PER_PX)
    fireEvent.pointerUp(el, { pointerId: 1, clientX: 140 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 200 })
    await frame()
    expect(onRotate).toHaveBeenCalledTimes(1)
  })

  it('ignores another button, and a pointer other than the one dragging', async () => {
    const { el, onRotate } = surface()
    fireEvent.pointerDown(el, { pointerId: 1, button: 2, clientX: 100 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 150 })
    fireEvent.pointerDown(el, { pointerId: 2, button: 0, clientX: 100 })
    fireEvent.pointerMove(el, { pointerId: 3, clientX: 150 })
    await frame()
    expect(onRotate).not.toHaveBeenCalled()
  })

  it('zooms in with the wheel up to its bound, out to the framing as it is, and gives the zoom back on unmount', async () => {
    const { el, onZoom, view } = surface()
    fireEvent.wheel(el, { deltaY: 100 })
    await frame()
    // Already at the framing as it is: nothing further out.
    expect(onZoom).not.toHaveBeenCalled()
    fireEvent.wheel(el, { deltaY: -200 })
    await waitFor(() => expect(onZoom).toHaveBeenCalled())
    const zoomed = onZoom.mock.calls.at(-1)![0] as number
    expect(zoomed).toBeGreaterThan(1)
    expect(zoomed).toBeLessThan(PREVIEW_MAX_ZOOM)
    for (let i = 0; i < 20; i++) fireEvent.wheel(el, { deltaY: -500 })
    await waitFor(() => expect(onZoom).toHaveBeenLastCalledWith(PREVIEW_MAX_ZOOM))
    view.unmount()
    expect(onZoom).toHaveBeenLastCalledWith(1)
  })

  it('gives back the zoom it last sent on unmount, even when a way back to 1 was still waiting for its frame', async () => {
    const { el, onZoom, view } = surface()
    fireEvent.wheel(el, { deltaY: -200 })
    await waitFor(() => expect(onZoom).toHaveBeenCalled())
    expect(onZoom.mock.calls.at(-1)![0]).toBeGreaterThan(1)
    // Back out to 1, then gone before that change is sent: the scene still has the zoom sent last.
    fireEvent.wheel(el, { deltaY: 2000 })
    view.unmount()
    expect(onZoom).toHaveBeenLastCalledWith(1)
  })

  it('leaves the zoom alone on unmount when it was never changed', () => {
    const { onZoom, view } = surface()
    view.unmount()
    expect(onZoom).not.toHaveBeenCalled()
  })
})
