import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { deleteItem, fetchContent, fetchContentSize, fetchItemContents, saveItem } from '~/lib/builder'
import { type ThumbnailPatch } from '~/components/ThumbnailModal'
import { type Collection } from '~/lib/collections'
import { ItemType, type Item, BODY_SHAPE_MALE } from '~/lib/items'
import { ConfirmItemsStep } from './ConfirmItemsStep'

vi.mock('~/lib/builder', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/builder')>()),
  saveItem: vi.fn(),
  deleteItem: vi.fn(),
  fetchContent: vi.fn(),
  fetchContentSize: vi.fn(),
  fetchItemContents: vi.fn()
}))

// Stored file sizes by content hash; anything else is a small file.
const STORED_SIZES: Record<string, number> = { Qmmodel: 2_800_000, Qmthumb: 200_000, Qmimage: 40_000 }

// The real editor renders a WearablePreview iframe; stand in with a button that hands back a fixed PNG.
const NEW_THUMBNAIL = new Blob(['png'], { type: 'image/png' })
vi.mock('~/components/ThumbnailModal', () => ({
  ThumbnailModal: ({
    contents,
    onSave,
    onClose
  }: {
    contents: Record<string, Blob> | null
    onSave: (patch: ThumbnailPatch) => void
    onClose: () => void
  }) => (
    <div data-testid="thumbnail-modal" data-loaded={contents ? true : undefined}>
      <button
        data-testid="thumbnail-capture"
        onClick={() =>
          onSave({
            thumbnail: 'data:image/png;base64,bmV3',
            contents: { ...contents, 'thumbnail.png': NEW_THUMBNAIL },
            isAutoThumbnail: false
          })
        }
      />
      <button data-testid="thumbnail-close" onClick={onClose} />
    </div>
  )
}))

const ADDRESS = '0x00000000000000000000000000000000000000aa'

function makeItem(id: string, name: string): Item {
  return {
    id,
    name,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: ADDRESS,
    collectionId: 'col-1',
    rarity: 'legendary',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: {
      category: 'upper_body',
      representations: [{ bodyShapes: [BODY_SHAPE_MALE], mainFile: 'm.glb', contents: ['m.glb'] }]
    },
    contents: { 'm.glb': 'Qmmodel', 'thumbnail.png': 'Qmthumb', 'image.png': 'Qmimage' },
    createdAt: 1,
    updatedAt: 1
  }
}

const items = [makeItem('a', 'Pirate Hat'), makeItem('b', 'Ghost Cape')]
const collection: Collection = {
  id: 'col-1',
  name: 'Pirate Hats',
  owner: ADDRESS,
  urn: 'urn:decentraland:amoy:collections-v2:0xc0ffee',
  isPublished: false,
  isApproved: false,
  itemCount: 2,
  minters: [],
  managers: ['0xmanager'],
  createdAt: 1,
  updatedAt: 1
}

function renderStep(list = items, address = ADDRESS, readOnly = false) {
  const onConfirm = vi.fn()
  const onBack = vi.fn()
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <TranslationProvider>{children}</TranslationProvider>
    </QueryClientProvider>
  )
  render(
    <ConfirmItemsStep
      address={address}
      collection={collection}
      items={list}
      readOnly={readOnly}
      onBusyChange={vi.fn()}
      onBack={onBack}
      onConfirm={onConfirm}
    />,
    { wrapper }
  )
  return { onConfirm, onBack }
}

beforeEach(() => {
  vi.mocked(saveItem).mockReset()
  vi.mocked(deleteItem).mockReset()
  vi.mocked(fetchItemContents).mockReset()
  vi.mocked(fetchContent).mockReset()
  vi.mocked(fetchContentSize).mockReset()
  vi.mocked(fetchContentSize).mockImplementation(async hash => STORED_SIZES[hash] ?? 1_000)
})

describe('ConfirmItemsStep', () => {
  it('lists every item with body type, category and rarity, and gates continue on the checkbox', async () => {
    const { onConfirm } = renderStep()
    const rows = screen.getAllByTestId('publish-item-row')
    expect(rows).toHaveLength(2)
    expect(within(rows[0]).getByTestId('publish-item-body-type')).toHaveTextContent('Male')
    expect(within(rows[0]).getByTestId('publish-item-category')).toHaveTextContent('Upper Body')
    expect(within(rows[0]).getByTestId('publish-item-rarity')).toHaveTextContent('Legendary (100)')

    const confirm = screen.getByTestId('publish-items-confirm')
    expect(confirm).toBeDisabled()
    await userEvent.click(screen.getByTestId('publish-items-accept'))
    await userEvent.click(confirm)
    expect(onConfirm).toHaveBeenCalled()
  })

  it('refuses a row edit that would take the item over its limit, with the reason', async () => {
    vi.mocked(saveItem).mockImplementation(async (_address, item) => item)
    // The new rarity image leaves no room: 2,800,000 + 200,000 + 200,000 bytes.
    vi.mocked(fetchContent).mockResolvedValue(new Blob([new Uint8Array(200_000)], { type: 'image/png' }))
    renderStep()
    const row = screen.getAllByTestId('publish-item-row')[0]
    await userEvent.click(within(row).getByTestId('publish-item-edit'))
    await userEvent.click(within(row).getByTestId('rarity-select'))
    await userEvent.click(screen.getByTestId('rarity-select-option-epic'))
    await userEvent.click(within(row).getByTestId('publish-item-save'))

    expect(await within(row).findByTestId('publish-item-error')).toHaveTextContent('over the 3.00 MB limit')
    expect(saveItem).not.toHaveBeenCalled()
  })

  it('saves an edited name and rarity as soon as the check button is clicked', async () => {
    vi.mocked(saveItem).mockImplementation(async (_address, item) => item)
    vi.mocked(fetchContent).mockResolvedValue(new Blob(['png'], { type: 'image/png' }))
    renderStep()
    const row = screen.getAllByTestId('publish-item-row')[0]
    await userEvent.click(within(row).getByTestId('publish-item-edit'))

    const input = within(row).getByTestId('publish-item-name-input')
    await userEvent.clear(input)
    await userEvent.type(input, 'Captain Hat')
    await userEvent.click(within(row).getByTestId('rarity-select'))
    await userEvent.click(screen.getByTestId('rarity-select-option-epic'))
    await userEvent.click(within(row).getByTestId('publish-item-save'))

    await waitFor(() => expect(saveItem).toHaveBeenCalledTimes(1))
    const [, saved, blobs] = vi.mocked(saveItem).mock.calls[0]
    expect(saved).toMatchObject({ id: 'a', name: 'Captain Hat', rarity: 'epic' })
    // A new rarity means a new catalyst image, rebuilt from the stored thumbnail.
    expect(fetchContent).toHaveBeenCalledWith('Qmthumb')
    expect(Object.keys(blobs ?? {})).toEqual(['image.png'])
    await waitFor(() => expect(within(row).queryByTestId('publish-item-name-input')).not.toBeInTheDocument())
  })

  it('does not save an empty name and lets the edit be cancelled', async () => {
    renderStep()
    const row = screen.getAllByTestId('publish-item-row')[1]
    await userEvent.click(within(row).getByTestId('publish-item-edit'))
    await userEvent.clear(within(row).getByTestId('publish-item-name-input'))
    expect(within(row).getByTestId('publish-item-save')).toBeDisabled()
    // Other rows are locked while one is being edited.
    expect(within(screen.getAllByTestId('publish-item-row')[0]).getByTestId('publish-item-edit')).toBeDisabled()
    expect(screen.getByTestId('publish-items-confirm')).toBeDisabled()

    await userEvent.click(within(row).getByTestId('publish-item-cancel'))
    expect(within(row).getByTestId('publish-item-name')).toHaveTextContent('Ghost Cape')
    expect(saveItem).not.toHaveBeenCalled()
  })

  it('offers delete to the collection owner only', () => {
    renderStep(items, '0xmanager')
    expect(screen.queryByTestId('publish-item-delete')).not.toBeInTheDocument()
  })

  it('deletes an item after confirmation', async () => {
    vi.mocked(deleteItem).mockResolvedValue(undefined)
    renderStep()
    await userEvent.click(within(screen.getAllByTestId('publish-item-row')[1]).getByTestId('publish-item-delete'))
    expect(screen.getByTestId('delete-item-modal-description')).toHaveTextContent('"Ghost Cape"')
    await userEvent.click(screen.getByTestId('delete-item-confirm'))
    await waitFor(() => expect(deleteItem).toHaveBeenCalledWith(ADDRESS, 'b'))
    await waitFor(() => expect(screen.queryByTestId('delete-item-modal')).not.toBeInTheDocument())
  })

  it('never offers to delete the last item', () => {
    renderStep([items[0]])
    expect(screen.queryByTestId('publish-item-delete')).not.toBeInTheDocument()
  })

  it('only confirms the items of a collection locked by a publish: no actions column', async () => {
    const { onConfirm } = renderStep(items, ADDRESS, true)
    expect(screen.getAllByTestId('publish-item-row')).toHaveLength(2)
    expect(screen.queryByTestId('publish-item-edit')).not.toBeInTheDocument()
    expect(screen.queryByTestId('publish-items-actions-column')).not.toBeInTheDocument()
    expect(screen.queryByTestId('publish-item-actions')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-items-accept'))
    await userEvent.click(screen.getByTestId('publish-items-confirm'))
    expect(onConfirm).toHaveBeenCalled()
  })

  it('cannot continue with no items', async () => {
    renderStep([])
    expect(screen.getByTestId('publish-items-empty')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-items-accept'))
    expect(screen.getByTestId('publish-items-confirm')).toBeDisabled()
  })
})

describe('ConfirmItemsStep thumbnail editing', () => {
  it('lets the creator pick a new thumbnail while editing and uploads it with the row save', async () => {
    vi.mocked(fetchItemContents).mockResolvedValue({ 'thumbnail.png': new Blob(['old']) })
    vi.mocked(saveItem).mockImplementation(async (_address, item) => item)
    renderStep()
    const row = screen.getAllByTestId('publish-item-row')[0]
    expect(within(row).queryByTestId('publish-item-edit-thumbnail')).not.toBeInTheDocument()

    await userEvent.click(within(row).getByTestId('publish-item-edit'))
    await userEvent.click(within(row).getByTestId('publish-item-edit-thumbnail'))
    await waitFor(() => expect(screen.getByTestId('thumbnail-modal')).toHaveAttribute('data-loaded'))
    await userEvent.click(screen.getByTestId('thumbnail-capture'))

    expect(screen.queryByTestId('thumbnail-modal')).not.toBeInTheDocument()
    expect(within(row).getByTestId('publish-item-img')).toHaveAttribute('src', 'data:image/png;base64,bmV3')
    expect(saveItem).not.toHaveBeenCalled()

    await userEvent.click(within(row).getByTestId('publish-item-save'))
    await waitFor(() => expect(saveItem).toHaveBeenCalledTimes(1))
    const [, saved, blobs] = vi.mocked(saveItem).mock.calls[0]
    expect(saved.thumbnail).toBe('thumbnail.png')
    expect(saved.contents['thumbnail.png']).toMatch(/^baf/)
    // The catalyst image is regenerated alongside a new thumbnail.
    expect(Object.keys(blobs).sort()).toEqual(['image.png', 'thumbnail.png'])
    expect(blobs['thumbnail.png']).toBe(NEW_THUMBNAIL)
  })

  it('drops a picked thumbnail when the row edit is cancelled', async () => {
    vi.mocked(fetchItemContents).mockResolvedValue({})
    renderStep()
    const row = screen.getAllByTestId('publish-item-row')[0]
    await userEvent.click(within(row).getByTestId('publish-item-edit'))
    await userEvent.click(within(row).getByTestId('publish-item-edit-thumbnail'))
    await userEvent.click(await screen.findByTestId('thumbnail-capture'))
    await userEvent.click(within(row).getByTestId('publish-item-cancel'))

    expect(within(row).getByTestId('publish-item-img')).toHaveAttribute('src', expect.stringContaining('Qmthumb'))
    expect(saveItem).not.toHaveBeenCalled()
  })
})
