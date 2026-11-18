import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { act, render, screen, waitFor } from '@testing-library/react'
import { PreviewRenderer } from '@dcl/schemas'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { ItemFileError, THUMBNAIL_PATH } from '~/lib/itemFiles'
import { ItemType } from '~/lib/items'
import { ThumbnailModal } from './ThumbnailModal'

let renderer: PreviewRenderer | undefined
vi.mock('~/hooks/usePreviewRenderer', () => ({ usePreviewRenderer: () => renderer }))

type PreviewProps = { id: string; unity?: boolean; onLoad?: () => void }
let previewProps: PreviewProps | null = null
const getScreenshot = vi.fn()
vi.mock('~/components/WearablePreview', () => ({
  WearablePreview(props: PreviewProps) {
    previewProps = props
    return <iframe id={props.id} title="preview" />
  },
  createWearablePreviewController: () => ({ scene: { getScreenshot }, emote: { pause: vi.fn() } }),
  EmoteControls: () => null,
  TranslationControls: () => null
}))
vi.mock('~/components/ZoomControls', () => ({ ZoomControls: () => null }))

// jsdom has no canvas: resizing hands back a fixed PNG data URL.
const PNG_DATA_URL = 'data:image/png;base64,iVBORw0KGgo='
vi.mock('~/lib/media', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/media')>()),
  getImageType: vi.fn().mockResolvedValue('png'),
  resizeImage: vi.fn().mockResolvedValue(new Blob(['png'], { type: 'image/png' })),
  blobToDataURL: vi.fn().mockResolvedValue('data:image/png;base64,iVBORw0KGgo=')
}))

function renderModal(contents: Record<string, Blob> | null = null, onSave = vi.fn()) {
  const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>
  render(<ThumbnailModal type={ItemType.WEARABLE} contents={contents} onSave={onSave} onClose={vi.fn()} />, {
    wrapper
  })
  return { onSave }
}

beforeEach(() => {
  renderer = undefined
  previewProps = null
  getScreenshot.mockReset()
})

describe('ThumbnailModal', () => {
  it('lets a picture be uploaded while the item files and preview are still loading', async () => {
    const { onSave } = renderModal()
    expect(screen.getByTestId('thumbnail-loading')).toBeInTheDocument()
    expect(screen.getByTestId('thumbnail-capture')).toBeDisabled()
    expect(screen.getByTestId('thumbnail-upload')).toBeEnabled()

    await userEvent.upload(
      screen.getByTestId('thumbnail-file-input'),
      new File(['png'], 'thumb.png', { type: 'image/png' })
    )

    await waitFor(() => expect(onSave).toHaveBeenCalled())
    const [patch] = onSave.mock.calls[0]
    expect(patch.thumbnail).toBe(PNG_DATA_URL)
    expect(patch.contents[THUMBNAIL_PATH]).toBeInstanceOf(Blob)
  })

  it('shows a spinner on the upload button and keeps the modal inert until the save settles', async () => {
    const { onSave } = renderModal()

    await userEvent.upload(
      screen.getByTestId('thumbnail-file-input'),
      new File(['png'], 'thumb.png', { type: 'image/png' })
    )

    await waitFor(() => expect(onSave).toHaveBeenCalled())
    const upload = screen.getByTestId('thumbnail-upload')
    expect(upload).toHaveAttribute('aria-busy', 'true')
    expect(upload).toBeDisabled()
    expect(screen.getByTestId('thumbnail-capture')).toBeDisabled()
  })

  it('stays open with the reason when the save fails, ready to try again', async () => {
    const onSave = vi.fn().mockRejectedValue(new ItemFileError('size_exceeded', { size: '3.63', limit: '3.00' }))
    renderModal(null, onSave)

    await userEvent.upload(
      screen.getByTestId('thumbnail-file-input'),
      new File(['png'], 'thumb.png', { type: 'image/png' })
    )

    expect(await screen.findByTestId('thumbnail-error')).toHaveTextContent(
      'This item is 3.63 MB, over the 3.00 MB limit (thumbnail included).'
    )
    expect(screen.getByTestId('thumbnail-upload')).toBeEnabled()
  })

  it('captures the Unity preview, waiting out a reload that overtook the capture', async () => {
    renderer = PreviewRenderer.UNITY
    getScreenshot
      .mockRejectedValueOnce(new Error('The preview reloaded during the capture'))
      .mockResolvedValue(PNG_DATA_URL)
    const { onSave } = renderModal({ 'hat.glb': new Blob(['glb']) })

    expect(previewProps?.unity).toBe(true)
    act(() => previewProps?.onLoad?.())
    await userEvent.click(screen.getByTestId('thumbnail-capture'))

    await waitFor(() => expect(onSave).toHaveBeenCalled(), { timeout: 3000 })
    expect(onSave.mock.calls[0][0]).toMatchObject({ thumbnail: PNG_DATA_URL, isAutoThumbnail: false })
    expect(screen.queryByTestId('thumbnail-error')).not.toBeInTheDocument()
  })
})
