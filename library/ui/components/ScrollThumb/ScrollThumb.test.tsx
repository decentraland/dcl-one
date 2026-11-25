import '@testing-library/jest-dom/vitest'
import { act, fireEvent, render } from '@testing-library/react'
import { useRef, useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ScrollThumb } from './ScrollThumb'

// jsdom lays nothing out: the scroller's heights are set by hand, as a browser would measure them.
function sized(el: HTMLElement, view: number, content: number): void {
  Object.defineProperty(el, 'clientHeight', { configurable: true, value: view })
  Object.defineProperty(el, 'scrollHeight', { configurable: true, value: content })
}

function Scroller({ swap }: { swap: { current: (long: boolean) => void } }) {
  const ref = useRef<HTMLDivElement>(null)
  const [long, setLong] = useState(false)
  swap.current = setLong
  return (
    <div>
      <div ref={ref} data-testid="scroller">{long ? <ul>{Array.from({ length: 40 }, (_, i) => <li key={i}>{i}</li>)}</ul> : <p>Nothing here</p>}</div>
      <ScrollThumb target={ref} length={50} />
    </div>
  )
}

/** Records what is observed, and lets a test report a resize. */
class ResizeObserverStub {
  static instances: ResizeObserverStub[] = []
  targets = new Set<Element>()
  constructor(private readonly callback: () => void) {
    ResizeObserverStub.instances.push(this)
  }
  observe(target: Element) {
    this.targets.add(target)
  }
  unobserve(target: Element) {
    this.targets.delete(target)
  }
  disconnect() {
    this.targets.clear()
  }
  resize() {
    this.callback()
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
  ResizeObserverStub.instances = []
})

describe('ScrollThumb', () => {
  it('is hidden from assistive technology and reports its axis and tone', () => {
    const swap = { current: (() => {}) as (long: boolean) => void }
    const { container } = render(<Scroller swap={swap} />)
    const track = container.querySelector('[aria-hidden="true"]') as HTMLElement
    expect(track).toHaveAttribute('data-axis', 'y')
    expect(track).toHaveAttribute('data-tone', 'outline')
    expect(track).not.toHaveAttribute('data-scrolls')
  })

  it('follows content swapped inside a scroller whose own box keeps its size', async () => {
    vi.stubGlobal('ResizeObserver', ResizeObserverStub)
    const swap = { current: (() => {}) as (long: boolean) => void }
    const { getByTestId, container } = render(<Scroller swap={swap} />)
    const scroller = getByTestId('scroller')
    const thumb = container.querySelector('[aria-hidden="true"] > span') as HTMLElement
    sized(scroller, 100, 100)
    Object.defineProperty(thumb.parentElement!, 'clientHeight', { configurable: true, value: 100 })
    // Everything fits: hidden.
    await act(async () => swap.current(false))
    expect(thumb.style.visibility).toBe('hidden')
    // The long content comes in with no scroll and no resize of the scroller.
    sized(scroller, 100, 400)
    await act(async () => swap.current(true))
    await act(async () => {})
    expect(thumb.style.visibility).toBe('visible')
    expect(thumb.style.height).toBe('50px')
  })

  it('scrolls by a drag of the pill and by a press on its track', async () => {
    vi.stubGlobal('ResizeObserver', ResizeObserverStub)
    const swap = { current: (() => {}) as (long: boolean) => void }
    const { getByTestId, container } = render(<Scroller swap={swap} />)
    const scroller = getByTestId('scroller')
    const thumb = container.querySelector('[aria-hidden="true"] > span') as HTMLElement
    const track = thumb.parentElement!
    // A 50px pill on a 100px track over 300px of hidden content: 6px of content for each pixel of the pill.
    sized(scroller, 100, 400)
    Object.defineProperty(track, 'clientHeight', { configurable: true, value: 100 })
    await act(async () => swap.current(true))
    await act(async () => {})
    expect(track.hasAttribute('data-scrolls')).toBe(true)

    fireEvent.pointerDown(thumb, { button: 0, pointerId: 1, clientY: 10 })
    fireEvent.pointerMove(track, { pointerId: 1, clientY: 35 })
    expect(scroller.scrollTop).toBe(150)
    // Past the end of the track it stops at the end of the content.
    fireEvent.pointerMove(track, { pointerId: 1, clientY: 500 })
    expect(scroller.scrollTop).toBe(300)
    fireEvent.pointerUp(track, { pointerId: 1, clientY: 500 })
    fireEvent.pointerMove(track, { pointerId: 1, clientY: 0 })
    expect(scroller.scrollTop).toBe(300)

    // A press on the track puts the pill's middle under the pointer.
    fireEvent.pointerDown(track, { button: 0, pointerId: 2, clientY: 40 })
    expect(scroller.scrollTop).toBe(90)
    fireEvent.pointerUp(track, { pointerId: 2, clientY: 40 })

    // Nothing to scroll: the track takes no press.
    sized(scroller, 100, 100)
    await act(async () => swap.current(false))
    await act(async () => {})
    expect(track.hasAttribute('data-scrolls')).toBe(false)
  })

  it('follows a swap of content where ResizeObserver does not exist', async () => {
    vi.stubGlobal('ResizeObserver', undefined)
    const swap = { current: (() => {}) as (long: boolean) => void }
    const { getByTestId, container } = render(<Scroller swap={swap} />)
    const scroller = getByTestId('scroller')
    const thumb = container.querySelector('[aria-hidden="true"] > span') as HTMLElement
    Object.defineProperty(thumb.parentElement!, 'clientHeight', { configurable: true, value: 100 })
    sized(scroller, 100, 400)
    await act(async () => swap.current(true))
    await act(async () => {})
    expect(thumb.style.visibility).toBe('visible')
  })

  it('watches the content that comes in, and follows a resize of it', async () => {
    vi.stubGlobal('ResizeObserver', ResizeObserverStub)
    const swap = { current: (() => {}) as (long: boolean) => void }
    const { getByTestId, container } = render(<Scroller swap={swap} />)
    const scroller = getByTestId('scroller')
    const thumb = container.querySelector('[aria-hidden="true"] > span') as HTMLElement
    const track = thumb.parentElement!
    Object.defineProperty(track, 'clientHeight', { configurable: true, value: 100 })
    const [observer] = ResizeObserverStub.instances
    expect(observer!.targets.has(scroller)).toBe(true)
    expect(observer!.targets.has(track)).toBe(true)

    sized(scroller, 100, 100)
    await act(async () => swap.current(true))
    await act(async () => {})
    const list = scroller.querySelector('ul')!
    expect(observer!.targets.has(list)).toBe(true)
    expect(thumb.style.visibility).toBe('hidden')

    // The list grows with no scroll and no change of children: only the observer reports it.
    sized(scroller, 100, 400)
    act(() => observer!.resize())
    expect(thumb.style.visibility).toBe('visible')

    await act(async () => swap.current(false))
    await act(async () => {})
    expect(observer!.targets.has(list)).toBe(false)
  })
})
