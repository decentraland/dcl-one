import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { TranslationProvider } from '~/intl'
import { BodyShapeType, ItemType, type Item } from '~/lib/items'
import { createDraft, type ItemDraft } from './AddItemsModal.state'
import { DraftForm } from './DraftForm'

vi.mock('~/hooks/useThumbnailValidation', () => ({ useThumbnailValidation: () => ({ data: undefined }) }))

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderForm(patch: Partial<ItemDraft>, extra: Partial<Parameters<typeof DraftForm>[0]> = {}) {
  const draft: ItemDraft = {
    ...createDraft(new File([''], 'jacket.glb')),
    status: 'ready',
    type: ItemType.WEARABLE,
    category: 'upper_body',
    bodyShape: BodyShapeType.MALE,
    thumbnail: 'data:image/png;base64,AAAA',
    metrics: { triangles: 708, materials: 3, meshes: 5, textures: 4, bodies: 5, entities: 1 },
    ...patch
  }
  const onOpenThumbnail = vi.fn()
  render(
    <DraftForm
      draft={draft}
      drafts={[draft]}
      collectionItems={[]}
      onUpdate={vi.fn()}
      onOpenThumbnail={onOpenThumbnail}
      onOpenVideo={vi.fn()}
      onVideoChange={vi.fn()}
      {...extra}
    />,
    { wrapper }
  )
  return { onOpenThumbnail }
}

describe('DraftForm thumbnail', () => {
  it('opens the thumbnail editor for a regular item', async () => {
    const { onOpenThumbnail } = renderForm({ isVariant: false })
    await userEvent.click(screen.getByTestId('edit-thumbnail'))
    expect(onOpenThumbnail).toHaveBeenCalled()
  })

  it('lets the creator pick a thumbnail while the auto one is still being generated', async () => {
    const { onOpenThumbnail } = renderForm({ isVariant: false, thumbnail: null, metrics: null })
    expect(screen.getByTestId('draft-thumbnail-loading')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('edit-thumbnail'))
    expect(onOpenThumbnail).toHaveBeenCalled()
  })

  it('only previews the thumbnail of a body shape variant, since its target keeps its own', () => {
    renderForm({ isVariant: true })
    expect(screen.getByTestId('draft-thumbnail')).toBeInTheDocument()
    expect(screen.queryByTestId('edit-thumbnail')).not.toBeInTheDocument()
  })

  it('holds the metrics row with placeholders until the preview has measured the model', () => {
    renderForm({ isVariant: false, metrics: null })
    expect(screen.getAllByTestId('draft-metrics-loading')).toHaveLength(3)
    expect(screen.queryByTestId('draft-thumbnail-loading')).not.toBeInTheDocument()
  })
})

describe('DraftForm in edit in bulk', () => {
  it('names the item the zip updates, with its URN identifier', () => {
    const urn = 'urn:decentraland:amoy:collections-thirdparty:brand:hats:gold-hat'
    renderForm(
      { urnSuffix: 'gold-hat' },
      {
        linked: { contract: null, updating: true },
        collectionItems: [{ id: 'i1', name: 'Gold Hat', urn } as Item]
      }
    )
    expect(screen.getByTestId('bulk-edit-target')).toHaveTextContent('Updates Gold Hat (gold-hat)')
    expect(screen.getByTestId('bulk-edit-target')).toHaveAttribute('title', urn)
  })
})

describe('DraftForm size', () => {
  it('shows the size the item will be saved with, against its limit', () => {
    renderForm({}, { size: { bytes: 3_093_299, limit: 3 * 1024 * 1024 } })
    expect(screen.getByTestId('draft-size')).toHaveTextContent('2.95 / 3.00 MB')
    expect(screen.queryByTestId('size-error')).not.toBeInTheDocument()
  })

  it('explains an item over its limit, thumbnail included', () => {
    renderForm({}, { size: { bytes: 3_243_672, limit: 3 * 1024 * 1024 } })
    expect(screen.getByTestId('draft-size')).toHaveAttribute('data-exceeded', 'true')
    expect(screen.getByTestId('size-error')).toHaveTextContent(
      'This item is 3.09 MB, over the 3.00 MB limit (thumbnail included). Reduce its textures or geometry.'
    )
  })
})
