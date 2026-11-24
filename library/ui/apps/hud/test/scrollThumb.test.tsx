import { act, fireEvent, render } from '@testing-library/react'
import { useRef, useState } from 'react'
import { describe, expect, it } from 'vitest'
import { ScrollThumb } from '../design'

// jsdom lays nothing out: the scroller's heights are set by hand, as a browser would measure them.
function sized(el: HTMLElement, view: number, content: number): void {
  Object.defineProperty(el, 'clientHeight', { configurable: true, value: view })
  Object.defineProperty(el, 'scrollHeight', { configurable: true, value: content })
}

function Scroller({ swap }: { swap: React.MutableRefObject<(long: boolean) => void> }): React.JSX.Element {
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

describe('ScrollThumb', () => {
  it('follows content swapped inside a scroller whose own box keeps its size', async () => {
    const swap = { current: (_: boolean) => {} }
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
    const swap = { current: (_: boolean) => {} }
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
})
