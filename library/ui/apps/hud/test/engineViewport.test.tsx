// What the engine's cutout reports: its rect and the display's pixel ratio, the focus box only
// when there is one, and null once it is gone.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useRef } from 'react'
import { render } from '@testing-library/react'
import { EngineViewport } from '../features/engine/EngineViewport'

const VIEWPORT = new DOMRect(10, 20, 300, 400)
const FOCUS = new DOMRect(60, 20, 200, 400)

/** The cutout with a focus box beside it, in the order the Backpack renders them. */
function WithFocus({ report }: { report: React.ComponentProps<typeof EngineViewport>['report'] }): React.JSX.Element {
  const focus = useRef<HTMLDivElement>(null)
  return (
    <>
      <EngineViewport region="avatarPreview" report={report} focus={focus} />
      <div ref={focus} data-focus="" />
    </>
  )
}

function measure(focus: DOMRect): void {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    return this.hasAttribute('data-focus') ? focus : VIEWPORT
  })
}

function observed(): () => void {
  const callbacks: Array<() => void> = []
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: () => void) {
        callbacks.push(callback)
      }
      observe(): void {}
      disconnect(): void {}
    }
  )
  return () => callbacks.forEach((callback) => callback())
}

describe('EngineViewport', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('reports its rect and the pixel ratio, and nothing else, without a focus box', () => {
    measure(FOCUS)
    const report = vi.fn()
    render(<EngineViewport region="signin" report={report} />)
    expect(report.mock.calls).toEqual([['signin', { x: 10, y: 20, width: 300, height: 400 }, window.devicePixelRatio]])
  })

  it('reports the focus box with the rect once the box is measured', () => {
    measure(FOCUS)
    const resize = observed()
    const report = vi.fn()
    render(<WithFocus report={report} />)
    // The box after the cutout is not attached at the cutout's first report: the observer's call brings it.
    resize()
    expect(report.mock.lastCall).toEqual(['avatarPreview', { x: 10, y: 20, width: 300, height: 400 }, window.devicePixelRatio, { x: 60, y: 20, width: 200, height: 400 }])
  })

  it('leaves out a focus box that has no height yet', () => {
    measure(new DOMRect(60, 20, 200, 0))
    const resize = observed()
    const report = vi.fn()
    render(<WithFocus report={report} />)
    resize()
    expect(report.mock.lastCall).toEqual(['avatarPreview', { x: 10, y: 20, width: 300, height: 400 }, window.devicePixelRatio])
  })

  it('clears the region when it unmounts', () => {
    const report = vi.fn()
    const view = render(<EngineViewport region="map" report={report} />)
    report.mockClear()
    view.unmount()
    expect(report.mock.calls).toEqual([['map', null]])
  })
})
