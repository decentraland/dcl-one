import { describe, it, expect, vi } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { PhotoModeSnapshot } from '../engine/protocol'
import type { PhotoModeState } from '../features/session/useEngineSession'
import { PhotoModeOverlay } from '../features/photoMode/PhotoModeOverlay'

// DOMAIN: the photo-mode overlay — the buttons reach the engine through the session, and the
// capture effect follows the session's last capture.
describe('photo mode overlay', () => {
  const snapshot = (over: Partial<PhotoModeSnapshot> = {}): PhotoModeSnapshot => ({
    active: true,
    hudHidden: false,
    cursorLocked: true,
    phase: 'idle',
    storageCurrent: 1,
    storageMax: 500,
    storageLoaded: true,
    event: 'none',
    eventSeq: 0,
    previewJpegBase64: '',
    message: '',
    ...over
  })
  const photoMode = (over: Partial<PhotoModeState> = {}): PhotoModeState => ({
    snapshot: snapshot(),
    active: true,
    capture: null,
    set: vi.fn(),
    toggle: vi.fn(),
    take: vi.fn(),
    ...over
  })

  it('Gallery closes photo mode before it opens the gallery, like the G key', async () => {
    const calls: string[] = []
    const mode = photoMode({ set: vi.fn((enable: boolean) => calls.push(`set:${enable}`)) })
    render(<PhotoModeOverlay photoMode={mode} onOpenGallery={() => calls.push('gallery')} />)
    await userEvent.click(screen.getByRole('button', { name: 'Gallery' }))
    expect(calls).toEqual(['set:false', 'gallery'])
  })

  it('Close and Shoot reach the engine', async () => {
    const mode = photoMode()
    render(<PhotoModeOverlay photoMode={mode} onOpenGallery={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: 'Shoot' }))
    expect(mode.take).toHaveBeenCalledTimes(1)
    await userEvent.click(screen.getByRole('button', { name: 'Close camera' }))
    expect(mode.set).toHaveBeenCalledWith(false)
  })

  it.each([
    ['a photo is capturing', { phase: 'capturing' }],
    ['a photo is encoding', { phase: 'encoding' }],
    ['a photo is uploading', { phase: 'uploading' }],
    ['the storage is not known yet', { storageLoaded: false }]
  ] as [string, Partial<PhotoModeSnapshot>][])('Shoot is disabled while %s', (_what, over) => {
    render(<PhotoModeOverlay photoMode={photoMode({ snapshot: snapshot(over) })} onOpenGallery={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Shoot' })).toBeDisabled()
  })

  it('a full storage replaces Shoot with the notice', () => {
    render(
      <PhotoModeOverlay photoMode={photoMode({ snapshot: snapshot({ storageCurrent: 500, storageMax: 500 }) })} onOpenGallery={vi.fn()} />
    )
    expect(screen.queryByRole('button', { name: 'Shoot' })).toBeNull()
    expect(screen.getByRole('alert').textContent).toMatch(/storage limit/)
  })

  it('a capture from before the overlay mounted is not replayed', () => {
    render(<PhotoModeOverlay photoMode={photoMode({ capture: { seq: 3, preview: 'AAAA' } })} onOpenGallery={vi.fn()} />)
    expect(screen.queryByTestId('photo-flash')).toBeNull()
    expect(screen.getByRole('button', { name: 'Shoot' })).toBeEnabled()
  })

  it('a capture plays the flash once per seq and blocks Shoot while it plays', () => {
    vi.useFakeTimers()
    try {
      const { rerender } = render(<PhotoModeOverlay photoMode={photoMode()} onOpenGallery={vi.fn()} />)
      expect(screen.queryByTestId('photo-flash')).toBeNull()
      rerender(<PhotoModeOverlay photoMode={photoMode({ capture: { seq: 1, preview: 'AAAA' } })} onOpenGallery={vi.fn()} />)
      expect(screen.getByTestId('photo-flash')).toBeTruthy()
      expect(screen.getByRole('button', { name: 'Shoot' })).toBeDisabled()
      act(() => vi.advanceTimersByTime(1200))
      rerender(<PhotoModeOverlay photoMode={photoMode({ capture: { seq: 1, preview: 'AAAA' } })} onOpenGallery={vi.fn()} />)
      expect(screen.queryByTestId('photo-flash')).toBeNull()
      expect(screen.getByRole('button', { name: 'Shoot' })).toBeEnabled()
      rerender(<PhotoModeOverlay photoMode={photoMode({ capture: { seq: 2, preview: 'BBBB' } })} onOpenGallery={vi.fn()} />)
      expect(screen.getByTestId('photo-flash')).toBeTruthy()
    } finally {
      vi.useRealTimers()
    }
  })
})
