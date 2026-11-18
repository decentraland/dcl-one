import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { MAX_VIDEO_FILE_SIZE } from '~/lib/itemFiles'
import { VideoModal } from './VideoModal'

const loadVideoMetadata = vi.hoisted(() => vi.fn())
vi.mock('~/lib/media', () => ({ loadVideoMetadata }))

// jsdom has no object URLs; the player and poster only need a stable string.
URL.createObjectURL = vi.fn(() => 'blob:video')
URL.revokeObjectURL = vi.fn()

function renderModal(video: Blob | null = null, viewOnly = false) {
  const onChange = vi.fn()
  const onClose = vi.fn()
  const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
  render(<VideoModal video={video} viewOnly={viewOnly} onChange={onChange} onClose={onClose} />, { wrapper })
  return { onChange, onClose }
}

async function pick(file: File) {
  await userEvent.upload(screen.getByTestId('video-modal-dropzone-input'), file, { applyAccept: false })
}

const mp4 = (size = 10, name = 'clip.mp4') => new File([new Uint8Array(size)], name, { type: 'video/mp4' })

describe('VideoModal', () => {
  beforeEach(() => {
    loadVideoMetadata.mockReset()
    loadVideoMetadata.mockResolvedValue({ duration: 12 })
  })

  it('accepts a valid mp4 and hands it back, then shows the player', async () => {
    const { onChange, onClose } = renderModal()
    expect(screen.getByTestId('video-modal-dropzone')).toBeInTheDocument()

    const file = mp4()
    await pick(file)

    await waitFor(() => expect(onChange).toHaveBeenCalledWith(file))
    expect(onClose).not.toHaveBeenCalled()
  })

  it('rejects other formats, oversized files and broken videos without calling onChange', async () => {
    const { onChange } = renderModal()

    await pick(new File(['x'], 'clip.mov'))
    expect(await screen.findByTestId('video-modal-dropzone-error')).toHaveTextContent(/MP4/)

    await pick(mp4(MAX_VIDEO_FILE_SIZE + 1))
    expect(await screen.findByTestId('video-modal-dropzone-error')).toHaveTextContent(/too big/)

    loadVideoMetadata.mockRejectedValueOnce(new Error('nope'))
    await pick(mp4())
    expect(await screen.findByTestId('video-modal-dropzone-error')).toHaveTextContent(/invalid or broken/)

    expect(onChange).not.toHaveBeenCalled()
  })

  it('plays an existing video and offers replace and done', async () => {
    const { onChange, onClose } = renderModal(mp4())
    expect(screen.getByTestId('video-modal-player')).toHaveAttribute('src', 'blob:video')

    await userEvent.click(screen.getByTestId('video-modal-replace'))
    expect(screen.getByTestId('video-modal-dropzone')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('video-modal-cancel'))
    expect(screen.getByTestId('video-modal-player')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('video-modal-done'))
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(onChange).not.toHaveBeenCalled()
  })

  it('plays without offering replace when watching', () => {
    renderModal(mp4(), true)
    expect(screen.getByTestId('video-modal-player')).toBeInTheDocument()
    expect(screen.queryByTestId('video-modal-replace')).toBeNull()
  })

  it('waits for the stored video instead of showing a watcher the dropzone', () => {
    renderModal(null, true)
    expect(screen.getByTestId('video-modal-loading')).toBeInTheDocument()
    expect(screen.queryByTestId('video-modal-dropzone')).toBeNull()
  })
})
