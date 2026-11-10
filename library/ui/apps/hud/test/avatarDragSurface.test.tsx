// The page's hold on the preview avatar: a drag turns it, the wheel zooms its camera within its bounds toward
// the pointer's height, and unmounting gives the turn and the zoom back.
import { describe, it, expect, vi } from 'vitest'
import { useLayoutEffect } from 'react'
import { fireEvent, render, waitFor } from '@testing-library/react'
import { AvatarDragSurface, DEGREES_PER_PX } from '../features/engine/AvatarDragSurface'
import { PREVIEW_MAX_ZOOM } from '@explorer-core/engine/protocol'

function surface(box?: () => DOMRect | null) {
  const onRotate = vi.fn()
  const onZoom = vi.fn()
  const view = render(<AvatarDragSurface onRotate={onRotate} onZoom={onZoom} view={box} />)
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
    await waitFor(() => expect(onZoom.mock.calls.at(-1)![0]).toBe(PREVIEW_MAX_ZOOM))
    view.unmount()
    expect(onZoom).toHaveBeenLastCalledWith(1, 0.5)
  })

  it('zooms toward the height the pointer is at, measured on the camera’s view', async () => {
    // The view runs from 100px to 500px down the screen: a wheel at 200px is a quarter of the way down it.
    const { el, onZoom } = surface(() => ({ top: 100, height: 400 }) as DOMRect)
    fireEvent.wheel(el, { deltaY: -200, clientY: 200 })
    await waitFor(() => expect(onZoom).toHaveBeenCalled())
    expect(onZoom.mock.calls.at(-1)![1]).toBe(0.25)
    // Past either end of the view, the anchor stays on its edge.
    fireEvent.wheel(el, { deltaY: -200, clientY: 900 })
    await waitFor(() => expect(onZoom.mock.calls.at(-1)![1]).toBe(1))
  })

  it('gives the turn back on unmount, so the next page starts facing the way it did', async () => {
    const { el, onRotate, view } = surface()
    fireEvent.pointerDown(el, { pointerId: 1, button: 0, clientX: 100 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 140 })
    await waitFor(() => expect(onRotate).toHaveBeenCalledWith(40 * DEGREES_PER_PX))
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 160 })
    await waitFor(() => expect(onRotate).toHaveBeenCalledTimes(2))
    view.unmount()
    // All 60px it turned, back the other way (neither of the two turns it sent on its own).
    expect(onRotate).toHaveBeenCalledTimes(3)
    expect(onRotate).toHaveBeenLastCalledWith(-(60 * DEGREES_PER_PX))
  })

  it('gives back the zoom it last sent on unmount, even when a way back to 1 was still waiting for its frame', async () => {
    const { el, onZoom, view } = surface()
    fireEvent.wheel(el, { deltaY: -200 })
    await waitFor(() => expect(onZoom).toHaveBeenCalled())
    expect(onZoom.mock.calls.at(-1)![0]).toBeGreaterThan(1)
    // Back out to 1, then gone before that change is sent: the scene still has the zoom sent last.
    fireEvent.wheel(el, { deltaY: 2000 })
    view.unmount()
    expect(onZoom).toHaveBeenLastCalledWith(1, 0.5)
  })

  it('gives its changes back before the next page reports its own preview (the lobby after the Backpack)', async () => {
    const log: string[] = []
    // As EngineViewport does: the page that takes over reports its region from its layout effect.
    function NextPage(): null {
      useLayoutEffect(() => {
        log.push('lobby:open')
      }, [])
      return null
    }
    function Host({ open }: { open: boolean }): React.JSX.Element {
      return open ? <AvatarDragSurface onRotate={(d) => log.push(`rotate:${d}`)} onZoom={(z) => log.push(`zoom:${z}`)} /> : <NextPage />
    }
    const view = render(<Host open />)
    const el = view.container.firstElementChild as HTMLElement
    fireEvent.pointerDown(el, { pointerId: 1, button: 0, clientX: 100 })
    fireEvent.pointerMove(el, { pointerId: 1, clientX: 140 })
    fireEvent.wheel(el, { deltaY: -200 })
    await waitFor(() => expect(log).toContain(`rotate:${40 * DEGREES_PER_PX}`))
    view.rerender(<Host open={false} />)
    // The Backpack's avatar is turned back and its zoom given back while it is still the one on show.
    expect(log.slice(-3)).toEqual([`rotate:${-(40 * DEGREES_PER_PX)}`, 'zoom:1', 'lobby:open'])
  })

  it('leaves the turn and the zoom alone on unmount when neither was changed', () => {
    const { onRotate, onZoom, view } = surface()
    view.unmount()
    expect(onRotate).not.toHaveBeenCalled()
    expect(onZoom).not.toHaveBeenCalled()
  })
})
