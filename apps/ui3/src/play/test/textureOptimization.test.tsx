import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import { dismissToast, showToast, ToastHost } from '../design'
import type { TextureOptimizationMessage } from '../engine/protocol'
import {
  formatBytes,
  showTextureOptimization,
  textureNotice,
  TEXTURE_TOAST_KEY
} from '../features/preview/textureOptimizationToast'
import { enterAsGuest, renderSession } from './harness'

const status = (over: Partial<TextureOptimizationMessage> = {}): TextureOptimizationMessage => ({
  kind: 'textureOptimization',
  sceneId: 'scene-1',
  state: 'running',
  done: 2,
  total: 5,
  failed: 0,
  bytesBefore: 4_404_019,
  bytesAfter: 1_153_434,
  enabled: true,
  replay: false,
  ...over
})

const show = (msg: TextureOptimizationMessage, reload: (scene: string) => void = () => undefined): void =>
  act(() => showTextureOptimization(msg, reload))

afterEach(() => {
  // enabled:false takes the toast down and forgets a progress toast
  act(() => showTextureOptimization(status({ enabled: false }), () => undefined))
  act(() => dismissToast('k'))
  vi.useRealTimers()
})

describe('Toast primitive', () => {
  it('replaces a keyed toast in place and keeps one with no duration', () => {
    vi.useFakeTimers()
    render(<ToastHost />)
    act(() => showToast('1 / 3', { key: 'k', duration: null }))
    act(() => showToast('2 / 3', { key: 'k', duration: null }))
    expect(screen.getAllByRole('status')).toHaveLength(1)
    expect(screen.getByRole('status')).toHaveTextContent('2 / 3')
    act(() => vi.advanceTimersByTime(60_000))
    expect(screen.getByRole('status')).toHaveTextContent('2 / 3')
    act(() => dismissToast('k'))
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('restarts the timer of a keyed toast it replaces', () => {
    vi.useFakeTimers()
    render(<ToastHost />)
    act(() => showToast('first', { key: 'k', duration: 1000 }))
    act(() => vi.advanceTimersByTime(800))
    act(() => showToast('second', { key: 'k', duration: 1000 }))
    act(() => vi.advanceTimersByTime(800))
    expect(screen.getByRole('status')).toHaveTextContent('second')
    act(() => vi.advanceTimersByTime(300))
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('runs the action, then goes; Dismiss only goes', () => {
    render(<ToastHost />)
    const onClick = vi.fn()
    act(() => showToast('Ready', { key: 'k', duration: null, dismissible: true, action: { label: 'Go', ariaLabel: 'Go now', onClick } }))
    fireEvent.click(screen.getByRole('button', { name: 'Go now' }))
    expect(onClick).toHaveBeenCalledOnce()
    expect(screen.queryByRole('status')).toBeNull()
    act(() => showToast('Ready', { key: 'k', duration: null, dismissible: true, action: { label: 'Go', onClick } }))
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(onClick).toHaveBeenCalledOnce()
    expect(screen.queryByRole('status')).toBeNull()
  })
})

describe('texture optimization notice', () => {
  it('formats sizes for people', () => {
    expect(formatBytes(950)).toBe('950 B')
    expect(formatBytes(12 * 1024)).toBe('12 KB')
    expect(formatBytes(4_404_019)).toBe('4.2 MB')
    expect(formatBytes(3 * 1024 ** 3)).toBe('3.0 GB')
  })

  it('reads each state', () => {
    expect(textureNotice(status())).toEqual({ kind: 'progress', text: 'Optimizing textures… 2 / 5' })
    expect(textureNotice(status({ state: 'ready', done: 5 }))).toEqual({
      kind: 'ready',
      text: 'Textures optimized · 4.2 MB → 1.1 MB — Reload to see them'
    })
    expect(textureNotice(status({ state: 'failed', failed: 3 }))).toEqual({
      kind: 'failed',
      text: "3 textures couldn't be optimized and stay as PNG"
    })
    expect(textureNotice(status({ state: 'failed', failed: 1 }))?.text).toBe("1 texture couldn't be optimized and stay as PNG")
    expect(textureNotice(status({ state: 'failed', failed: 0 }))?.text).toBe(
      "Textures couldn't be optimized; the preview keeps serving the last optimized files"
    )
  })

  it('shows nothing for a project that does not serve optimized files', () => {
    for (const state of ['running', 'ready', 'failed'] as const) {
      expect(textureNotice(status({ state, enabled: false }))).toBeNull()
    }
  })

  it('ignores a READY replayed at connect, not a RUNNING or FAILED one', () => {
    expect(textureNotice(status({ state: 'ready', replay: true }))).toBeUndefined()
    expect(textureNotice(status({ replay: true }))?.kind).toBe('progress')
    expect(textureNotice(status({ state: 'failed', failed: 2, replay: true }))?.kind).toBe('failed')
  })
})

describe('texture optimization toast', () => {
  it('updates one busy progress toast in place, then offers Reload', () => {
    render(<ToastHost />)
    const reload = vi.fn()
    show(status({ done: 1 }), reload)
    show(status({ done: 4 }), reload)
    const progress = screen.getByRole('status')
    expect(screen.getAllByRole('status')).toHaveLength(1)
    expect(progress).toHaveTextContent('Optimizing textures… 4 / 5')
    expect(progress).toHaveAttribute('aria-busy', 'true')
    expect(screen.queryByRole('button', { name: /reload/i })).toBeNull()

    show(status({ state: 'ready', done: 5 }), reload)
    const ready = screen.getByRole('status')
    expect(ready).toHaveTextContent('Textures optimized · 4.2 MB → 1.1 MB — Reload to see them')
    expect(ready).not.toHaveAttribute('aria-busy')
    fireEvent.click(screen.getByRole('button', { name: 'Reload scene to see optimized textures' }))
    expect(reload).toHaveBeenCalledWith('scene-1')
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('reports a failure with Dismiss and no action', () => {
    render(<ToastHost />)
    show(status({ state: 'failed', failed: 3 }))
    expect(screen.getByRole('status')).toHaveTextContent("3 textures couldn't be optimized and stay as PNG")
    expect(screen.queryByRole('button', { name: /reload/i })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }))
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('takes the progress down when the project turns optimization off', () => {
    render(<ToastHost />)
    show(status())
    show(status({ state: 'ready', enabled: false }))
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('keeps an offered Reload when the reconnecting socket replays the same READY', () => {
    render(<ToastHost />)
    show(status({ state: 'ready' }))
    show(status({ state: 'ready', replay: true }))
    expect(screen.getByRole('button', { name: 'Reload scene to see optimized textures' })).toBeInTheDocument()
  })

  it('shows nothing for a READY replayed at connect, and clears progress left from before', () => {
    render(<ToastHost />)
    show(status({ state: 'ready', replay: true }))
    expect(screen.queryByRole('status')).toBeNull()
    show(status())
    show(status({ state: 'ready', replay: true }))
    expect(screen.queryByRole('status')).toBeNull()
  })
})

describe('texture optimization in the session', () => {
  it('Reload asks the bridge to reload the status scene', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<ToastHost />)
    h.driver.emit(status({ state: 'ready', sceneId: 'bafkpreview' }))
    fireEvent.click(screen.getByRole('button', { name: 'Reload scene to see optimized textures' }))
    expect(h.driver.last('reloadScene')).toEqual({ kind: 'reloadScene', scene: 'bafkpreview' })
  })

  it('a READY replayed at connect posts nothing and shows nothing', async () => {
    const h = renderSession()
    await enterAsGuest(h)
    render(<ToastHost />)
    h.driver.emit(status({ state: 'ready', replay: true }))
    expect(screen.queryByRole('status')).toBeNull()
    expect(h.driver.sentOf('reloadScene')).toHaveLength(0)
  })
})
