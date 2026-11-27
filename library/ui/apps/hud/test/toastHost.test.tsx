import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent, act, within } from '@testing-library/react'
import { ToastHost, showToast, dismissToast } from '../design'

// The toast queue is the HUD's: one module list, its timers and the keys. ToastHost draws each item with the
// shared Toast, whose own timer is off (`duration={0}`). These tests pin what the queue owns: its order, one
// toast per key, the timers, and the toasts that stay until acted on.

const notices = (): string[] => screen.queryAllByRole('status').map((t) => t.textContent ?? '')

beforeEach(() => {
  vi.useFakeTimers()
})

// The queue is a module list with no reset: every test names its lasting toasts by key and takes them down here,
// and the timers take down the rest.
const KEYS = ['progress', 'other', 'offer']
afterEach(() => {
  act(() => {
    KEYS.forEach(dismissToast)
    vi.runAllTimers()
  })
  vi.useRealTimers()
})

describe('the queue', () => {
  it('draws the toasts in the order they were shown, in one polite live region', () => {
    const { container } = render(<ToastHost />)
    act(() => {
      showToast('first')
      showToast('second', { tone: 'error' })
      showToast('third')
    })
    expect(notices()).toEqual(['first', 'second', 'third'])
    const host = container.firstElementChild as HTMLElement
    expect(host).toHaveAttribute('aria-live', 'polite')
    expect(host.children).toHaveLength(3)
  })

  it('draws nothing with no toast, and a toast shown before the host mounts once it does', () => {
    act(() => {
      showToast('early')
    })
    const { container } = render(<ToastHost />)
    expect(notices()).toEqual(['early'])
    act(() => vi.advanceTimersByTime(5000))
    expect(notices()).toEqual([])
    expect(container.firstElementChild).toBeEmptyDOMElement()
  })

  it('takes each toast down after its own time: 5 seconds unless it says otherwise', () => {
    render(<ToastHost />)
    act(() => {
      showToast('default')
      showToast('short', { duration: 1000 })
      showToast('long', { duration: 8000 })
    })
    act(() => vi.advanceTimersByTime(1000))
    expect(notices()).toEqual(['default', 'long'])
    act(() => vi.advanceTimersByTime(4000))
    expect(notices()).toEqual(['long'])
    act(() => vi.advanceTimersByTime(3000))
    expect(notices()).toEqual([])
  })

  it('keeps the same elements mounted for the toasts that stay when another leaves', () => {
    render(<ToastHost />)
    act(() => {
      showToast('leaves', { duration: 1000 })
      showToast('stays')
    })
    const stays = screen.getByText('stays')
    act(() => vi.advanceTimersByTime(1000))
    expect(screen.getByText('stays')).toBe(stays)
  })
})

describe('a keyed toast', () => {
  it('replaces the toast shown under its key in place: one per key, where the first one was', () => {
    render(<ToastHost />)
    act(() => {
      showToast('before')
      showToast('1 / 3', { key: 'progress', duration: null, busy: true })
      showToast('after')
    })
    const region = screen.getByText('1 / 3').closest('[role="status"]') as HTMLElement
    expect(region).toHaveAttribute('aria-busy', 'true')

    act(() => showToast('2 / 3', { key: 'progress', duration: null, busy: true }))
    act(() => showToast('done', { key: 'progress', duration: null }))
    expect(notices()).toEqual(['before', 'done', 'after'])
    // the same toast changed its text: it was not taken down and shown again
    expect(screen.getByText('done').closest('[role="status"]')).toBe(region)
    expect(region).not.toHaveAttribute('aria-busy')
  })

  it('keeps a toast under another key apart', () => {
    render(<ToastHost />)
    act(() => {
      showToast('a', { key: 'progress', duration: null })
      showToast('b', { key: 'other', duration: null })
      showToast('a2', { key: 'progress', duration: null })
    })
    expect(notices()).toEqual(['a2', 'b'])
  })

  it('counts its time from the last change, and stays once a change says so', () => {
    render(<ToastHost />)
    act(() => showToast('timed', { key: 'progress', duration: 2000 }))
    act(() => vi.advanceTimersByTime(1500))
    act(() => showToast('timed again', { key: 'progress', duration: 2000 }))
    act(() => vi.advanceTimersByTime(1500))
    expect(notices()).toEqual(['timed again'])
    act(() => vi.advanceTimersByTime(500))
    expect(notices()).toEqual([])

    act(() => showToast('timed', { key: 'progress', duration: 2000 }))
    act(() => showToast('held', { key: 'progress', duration: null }))
    act(() => vi.advanceTimersByTime(60_000))
    expect(notices()).toEqual(['held'])
  })

  it('is taken down by dismissToast, which does nothing for a key that is not shown', () => {
    render(<ToastHost />)
    act(() => {
      showToast('keyed', { key: 'progress', duration: null })
      showToast('plain')
    })
    act(() => dismissToast('never shown'))
    expect(notices()).toEqual(['keyed', 'plain'])
    act(() => dismissToast('progress'))
    act(() => dismissToast('progress'))
    expect(notices()).toEqual(['plain'])
  })
})

describe('a toast that stays', () => {
  it('outlives the timed toasts around it until it is dismissed', () => {
    render(<ToastHost />)
    act(() => {
      showToast('timed')
      showToast('Reload to see them', { key: 'offer', duration: null, dismissible: true })
      showToast('timed too')
    })
    act(() => vi.advanceTimersByTime(60_000))
    expect(notices()).toEqual(['Reload to see them'])
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(notices()).toEqual([])
  })

  it('runs its action once and goes away with it', () => {
    render(<ToastHost />)
    const onClick = vi.fn()
    act(() => {
      showToast('Textures optimized', {
        key: 'offer',
        duration: null,
        dismissible: true,
        action: { label: 'Reload', ariaLabel: 'Reload the scene', onClick }
      })
    })
    const toast = screen.getByRole('status')
    expect(within(toast).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(['Reload the scene', 'Dismiss'])
    fireEvent.click(within(toast).getByRole('button', { name: 'Reload the scene' }))
    expect(onClick).toHaveBeenCalledTimes(1)
    expect(notices()).toEqual([])
  })

  it('does not run its action when it is dismissed', () => {
    render(<ToastHost />)
    const onClick = vi.fn()
    act(() => {
      showToast('Textures optimized', { key: 'offer', duration: null, dismissible: true, action: { label: 'Reload', onClick } })
    })
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(onClick).not.toHaveBeenCalled()
    expect(notices()).toEqual([])
  })

  it('names its action by its label when it has no other name, and offers no Dismiss unless asked', () => {
    render(<ToastHost />)
    act(() => {
      showToast('Saved', { key: 'offer', duration: null, action: { label: 'Undo', onClick: () => undefined } })
    })
    expect(screen.getAllByRole('button').map((b) => b.textContent)).toEqual(['Undo'])
    expect(screen.getByRole('button', { name: 'Undo' })).not.toHaveAttribute('aria-label')
  })
})

describe('what a toast draws', () => {
  it('has no button without an action or a Dismiss, and takes a leading visual', () => {
    render(<ToastHost />)
    act(() => {
      showToast('plain')
      showToast('with a picture', { icon: <img alt="Mojito" src="data:," /> })
    })
    expect(screen.queryByRole('button')).toBeNull()
    expect(screen.getByRole('img', { name: 'Mojito' })).toBeInTheDocument()
  })

  it('keeps a long text and rich content whole', () => {
    render(<ToastHost />)
    const long = 'A very long notice. '.repeat(80).trim()
    act(() => {
      showToast(long)
      showToast(
        <>
          <b>Mojito</b> is online
        </>
      )
    })
    expect(notices()).toEqual([long, 'Mojito is online'])
  })

  it('keeps its place for empty content, which draws no notice', () => {
    render(<ToastHost />)
    act(() => {
      showToast('')
      showToast('next')
    })
    expect(notices()).toEqual(['', 'next'])
    expect(screen.getAllByRole('status')[0]).toBeEmptyDOMElement()
  })
})
