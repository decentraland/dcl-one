import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { Avatar } from '../design/Avatar'

// PRIMITIVE: Avatar — a face that fails falls back to the initial and is probed again a few times:
// a fresh profile's snapshot URL is published before the server has rendered the picture. The
// failure is shared by URL, so every Avatar showing it comes back together.
// (The failure store is module state: each test uses its own URLs.)
class FakeImage {
  static probes: FakeImage[] = []
  onload: (() => void) | null = null
  onerror: (() => void) | null = null
  set src(_: string) {
    FakeImage.probes.push(this)
  }
}

describe('Avatar', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    FakeImage.probes = []
    vi.stubGlobal('Image', FakeImage)
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  const imgs = (): HTMLElement[] => screen.queryAllByRole('img')
  const nextProbe = (): FakeImage => {
    act(() => vi.advanceTimersByTime(60_000))
    const probe = FakeImage.probes.shift()
    expect(probe).toBeDefined()
    return probe!
  }

  it('brings a failed picture back everywhere it is shown once a probe loads it', () => {
    render(
      <>
        <Avatar src="https://faces/a.png" name="robtfm" />
        <Avatar src="https://faces/a.png" name="robtfm" />
      </>
    )
    fireEvent.error(imgs()[0])
    expect(imgs()).toHaveLength(0)
    expect(screen.getAllByText('R')).toHaveLength(2)

    act(() => nextProbe().onerror?.())
    expect(imgs()).toHaveLength(0)

    act(() => nextProbe().onload?.())
    expect(imgs()).toHaveLength(2)
  })

  it('gives up after its retries, keeping the initial, and the next Avatar to mount tries again', () => {
    render(<Avatar src="https://faces/b.png" name="robtfm" />)
    fireEvent.error(imgs()[0])
    for (let attempt = 0; attempt < 5; attempt++) act(() => nextProbe().onerror?.())
    act(() => vi.advanceTimersByTime(600_000))
    expect(FakeImage.probes).toHaveLength(0)
    expect(imgs()).toHaveLength(0)

    render(<Avatar src="https://faces/b.png" name="robtfm" />)
    expect(imgs()).toHaveLength(1)
  })

  it('probes once when a picture fails again after an <img> loaded it', () => {
    render(
      <>
        <Avatar src="https://faces/e.png" name="robtfm" />
        <Avatar src="https://faces/e.png" name="robtfm" />
      </>
    )
    // the second <img> finishes loading before the failure has taken both down
    const [first, second] = imgs()
    act(() => {
      fireEvent.error(first)
      fireEvent.load(second)
    })
    fireEvent.error(imgs()[0])
    act(() => vi.advanceTimersByTime(60_000))
    expect(FakeImage.probes).toHaveLength(1)
  })

  it('shows a different picture straight away', () => {
    const { rerender } = render(<Avatar src="https://faces/c.png" name="robtfm" />)
    fireEvent.error(imgs()[0])
    expect(imgs()).toHaveLength(0)
    rerender(<Avatar src="https://faces/d.png" name="robtfm" />)
    expect(imgs()[0].getAttribute('src')).toBe('https://faces/d.png')
  })
})
