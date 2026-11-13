import { act, render } from '@testing-library/react'
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
})
