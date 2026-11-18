import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { pickFile } from '~/lib/filePicker'
import { blobToDataURL, getImageType, resizeImage } from '~/lib/media'
import { useNotifications } from '~/lib/notifications'
import { THUMBNAIL_PATH } from '~/lib/itemFiles'
import { ItemType, type Item } from '~/lib/items'
import { useThumbnailEditor, type ThumbnailSubject } from './useThumbnailEditor'

vi.mock('~/lib/filePicker', () => ({ pickFile: vi.fn() }))
vi.mock('~/lib/builder', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/builder')>()),
  fetchItemContents: vi.fn(() => new Promise(() => {}))
}))
// jsdom has no canvas: resizing hands back a fixed PNG data URL.
vi.mock('~/lib/media', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/media')>()),
  getImageType: vi.fn().mockResolvedValue('png'),
  resizeImage: vi.fn().mockResolvedValue(new Blob(['png'], { type: 'image/png' })),
  blobToDataURL: vi.fn().mockResolvedValue('data:image/png;base64,iVBORw0KGgo=')
}))

function makeItem(contents: Record<string, string>): Item {
  return {
    id: 'i1',
    name: 'Eyes',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'eyes', representations: [] },
    contents,
    createdAt: 1,
    updatedAt: 1
  }
}

function Harness({ subject, onSave }: { subject: ThumbnailSubject; onSave: () => void }) {
  const editor = useThumbnailEditor(onSave)
  return (
    <>
      <button data-testid="edit" onClick={() => editor.edit(subject)} />
      {editor.modal}
    </>
  )
}

function renderEditor(subject: ThumbnailSubject) {
  const onSave = vi.fn()
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
  render(<Harness subject={subject} onSave={onSave} />, { wrapper })
  return { onSave }
}

beforeEach(() => {
  vi.mocked(pickFile)
    .mockReset()
    .mockResolvedValue(new File(['png'], 'eyes.png', { type: 'image/png' }))
  vi.mocked(getImageType).mockResolvedValue('png' as never)
  vi.mocked(resizeImage).mockResolvedValue(new Blob(['png'], { type: 'image/png' }))
  vi.mocked(blobToDataURL).mockResolvedValue('data:image/png;base64,iVBORw0KGgo=')
  useNotifications.setState({ toasts: [] })
})

describe('useThumbnailEditor', () => {
  it('takes a texture-only wearable thumbnail straight from the file picker', async () => {
    const { onSave } = renderEditor({ kind: 'item', item: makeItem({ 'eyes.png': 'Qm1', 'thumbnail.png': 'Qm2' }) })
    await userEvent.click(screen.getByTestId('edit'))

    await waitFor(() => expect(onSave).toHaveBeenCalled())
    expect(onSave.mock.calls[0][0].contents[THUMBNAIL_PATH]).toBeInstanceOf(Blob)
    expect(screen.queryByTestId('thumbnail-modal')).not.toBeInTheDocument()
  })

  it('treats an in-memory texture draft the same way', async () => {
    const { onSave } = renderEditor({
      kind: 'files',
      id: 'd1',
      type: ItemType.WEARABLE,
      contents: { 'mouth.png': new Blob(['x']) }
    })
    await userEvent.click(screen.getByTestId('edit'))

    await waitFor(() => expect(onSave).toHaveBeenCalled())
    expect(screen.queryByTestId('thumbnail-modal')).not.toBeInTheDocument()
  })

  it('opens the posing modal for a wearable with a 3D model', async () => {
    const { onSave } = renderEditor({ kind: 'item', item: makeItem({ 'hat.glb': 'Qm1', 'thumbnail.png': 'Qm2' }) })
    await userEvent.click(screen.getByTestId('edit'))

    expect(screen.getByTestId('thumbnail-modal')).toBeInTheDocument()
    expect(pickFile).not.toHaveBeenCalled()
    expect(onSave).not.toHaveBeenCalled()
  })

  it('does nothing when the file picker is dismissed', async () => {
    vi.mocked(pickFile).mockResolvedValue(null)
    const { onSave } = renderEditor({ kind: 'item', item: makeItem({ 'eyes.png': 'Qm1' }) })
    await userEvent.click(screen.getByTestId('edit'))

    await waitFor(() => expect(pickFile).toHaveBeenCalled())
    expect(onSave).not.toHaveBeenCalled()
    expect(useNotifications.getState().toasts).toEqual([])
  })

  it('explains a picked file that is not a PNG instead of saving it', async () => {
    vi.mocked(getImageType).mockResolvedValue('jpeg' as never)
    const { onSave } = renderEditor({ kind: 'item', item: makeItem({ 'eyes.png': 'Qm1' }) })
    await userEvent.click(screen.getByTestId('edit'))

    await waitFor(() => expect(useNotifications.getState().toasts).toHaveLength(1))
    expect(useNotifications.getState().toasts[0].type).toBe('error')
    expect(onSave).not.toHaveBeenCalled()
  })

  it('hands a picture saved from the modal back with the subject it was opened for', async () => {
    // Its files never finish loading here, so no preview mounts; uploading doesn't need them.
    const subject: ThumbnailSubject = { kind: 'item', item: makeItem({ 'hat.glb': 'Qm1', 'thumbnail.png': 'Qm2' }) }
    const { onSave } = renderEditor(subject)
    await userEvent.click(screen.getByTestId('edit'))
    await userEvent.upload(
      screen.getByTestId('thumbnail-file-input'),
      new File(['png'], 't.png', { type: 'image/png' })
    )

    await waitFor(() => expect(onSave).toHaveBeenCalled())
    expect(onSave.mock.calls[0][1]).toBe(subject)
    await waitFor(() => expect(screen.queryByTestId('thumbnail-modal')).not.toBeInTheDocument())
  })
})
