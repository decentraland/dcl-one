import { beforeEach, describe, expect, it, vi } from 'vitest'
import { FeatureFlag } from '~/lib/featureFlags'
import { setFeatureFlags } from '~/test/featureFlags'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BodyShape } from '@dcl/schemas'
import { Providers } from '~/components/CollectionDetailPage/SellItemFlow/testUtils'
import { ItemType, type Item } from '~/lib/items'
import { useItemForm } from '../useItemForm'
import { type ItemFileChange } from '~/lib/itemDraft'
import { type ItemSize } from '~/lib/itemSize'
import { useNotifications } from '~/lib/notifications'
import { PropertiesPanel } from './PropertiesPanel'

// The change-file modal has its own spec; here it hands back whatever result the test prepared.
const fileChange = vi.hoisted<{ result: ItemFileChange | null }>(() => ({ result: null }))
vi.mock('../ChangeFileModal', () => ({
  ChangeFileModal: ({ item, onSave }: { item: Item; onSave: (result: ItemFileChange) => Promise<void> }) => (
    <button
      type="button"
      data-testid="mock-change-file-apply"
      data-shapes={item.data.representations.length}
      onClick={() => fileChange.result && void onSave(fileChange.result)}
    />
  )
}))
const builder = vi.hoisted(() => ({ fetchItemContents: vi.fn() }))
vi.mock('~/lib/builder', async () => ({
  ...(await vi.importActual<typeof import('~/lib/builder')>('~/lib/builder')),
  fetchContent: () => Promise.resolve(new Blob()),
  fetchItemContents: builder.fetchItemContents
}))

const campaign = vi.hoisted(() => ({ current: null as { name: string; mainTag: string } | null }))
vi.mock('~/hooks/useCampaign', () => ({ useCampaign: () => campaign.current }))

vi.mock('~/lib/featureFlags', async () => {
  const actual = await vi.importActual<typeof import('~/lib/featureFlags')>('~/lib/featureFlags')
  const mock = await import('~/test/featureFlags')
  return { ...actual, getIsFeatureEnabled: mock.getIsFeatureEnabled }
})

beforeEach(() => {
  setFeatureFlags(FeatureFlag.WEARABLE_UTILITY, FeatureFlag.VRM_OPTOUT)
  campaign.current = null
  // jsdom's createObjectURL only accepts its own Blob class.
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:thumbnail')
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined)
})

const item: Item = {
  id: 'w1',
  name: 'Hat',
  description: 'A hat',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  collectionId: 'c1',
  rarity: 'epic',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    hides: ['hair'],
    replaces: ['helmet'],
    tags: ['cool'],
    representations: [{ bodyShapes: [BodyShape.MALE], mainFile: 'male/hat.glb', contents: ['male/hat.glb'] }]
  },
  contents: { 'male/hat.glb': 'bafyglb', 'thumbnail.png': 'bafythumb' },
  createdAt: 1,
  updatedAt: 1
}

function Harness({
  editable = true,
  canDownload = true,
  readOnlyReason,
  onSave = vi.fn(),
  onSaveFile = () => Promise.resolve(),
  isSaving = false,
  subject = item,
  size = null
}: {
  editable?: boolean
  canDownload?: boolean
  readOnlyReason?: 'role' | 'lock'
  onSave?: () => void
  onSaveFile?: (change: ItemFileChange) => Promise<void>
  isSaving?: boolean
  subject?: Item
  size?: ItemSize | null
}) {
  const form = useItemForm(subject)
  return (
    <PropertiesPanel
      item={subject}
      address="0xabc"
      editable={editable}
      readOnlyReason={readOnlyReason}
      canDelete={editable}
      canDownload={canDownload}
      draft={form.draft}
      dispatch={form.dispatch}
      isDirty={form.isDirty}
      isSaving={isSaving}
      size={size}
      springBones={null}
      onSave={onSave}
      onSaveFile={onSaveFile}
      onSaveThumbnail={() => Promise.resolve()}
      onRevert={() => form.reset(subject)}
      onDeleted={vi.fn()}
    />
  )
}

describe('PropertiesPanel', () => {
  it('starts clean with the item values and shows Save/Revert only once something changes', async () => {
    render(<Harness />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-name')).toHaveValue('Hat')
    expect(screen.queryByTestId('properties-panel-footer')).not.toBeInTheDocument()
    await userEvent.type(screen.getByTestId('properties-panel-name'), ' 2')
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
    await userEvent.click(screen.getByTestId('properties-panel-revert'))
    expect(screen.getByTestId('properties-panel-name')).toHaveValue('Hat')
    expect(screen.queryByTestId('properties-panel-footer')).not.toBeInTheDocument()
  })

  it('folds legacy replaces into the hidden categories and clears them for a skin', async () => {
    render(<Harness />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-hides-categories')).toHaveTextContent('Hair, Helmet')
    await userEvent.click(screen.getByTestId('properties-panel-category'))
    await userEvent.click(screen.getByTestId('properties-panel-category-option-skin'))
    expect(screen.getByTestId('properties-panel-hides-categories')).toHaveTextContent('None')
  })

  it('keeps the file and delete actions in the header menu', async () => {
    render(<Harness />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-header')).toHaveTextContent('Hat')
    await userEvent.click(screen.getByTestId('properties-panel-actions'))
    expect(screen.getByTestId('properties-panel-download')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-change-file')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-add-representation')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-delete')).toBeInTheDocument()
  })

  it('keeps the download for a read-only viewer who may still have the files', async () => {
    render(<Harness editable={false} />, { wrapper: Providers })
    await userEvent.click(screen.getByTestId('properties-panel-actions'))
    expect(screen.getByTestId('properties-panel-download')).toBeInTheDocument()
    expect(screen.queryByTestId('properties-panel-change-file')).not.toBeInTheDocument()
  })

  it('drops the download and the empty menu for someone outside the collection', () => {
    render(<Harness editable={false} canDownload={false} />, { wrapper: Providers })
    expect(screen.queryByTestId('properties-panel-actions')).not.toBeInTheDocument()
    expect(screen.queryByTestId('properties-panel-download')).not.toBeInTheDocument()
  })

  it('tells the creator a download failed even after they left the panel', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    let fail: (error: Error) => void = () => undefined
    builder.fetchItemContents.mockReturnValue(new Promise((_, reject) => (fail = reject)))
    const { unmount } = render(<Harness />, { wrapper: Providers })
    await userEvent.click(screen.getByTestId('properties-panel-actions'))
    await userEvent.click(screen.getByTestId('properties-panel-download'))
    unmount()
    fail(new Error('storage down'))
    await vi.waitFor(() =>
      expect(useNotifications.getState().toasts).toContainEqual(
        expect.objectContaining({ type: 'error', message: "We couldn't download the item. Please try again." })
      )
    )
  })

  it('exposes the wearable options as switches', async () => {
    render(<Harness />, { wrapper: Providers })
    const vrm = await screen.findByTestId('properties-panel-vrm-export')
    expect(vrm).toHaveAttribute('aria-checked', 'true')
    await userEvent.click(vrm)
    expect(vrm).toHaveAttribute('aria-checked', 'false')
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('drops the utility field and the VRM switch while their feature flags are off', async () => {
    setFeatureFlags()
    render(<Harness />, { wrapper: Providers })
    // The outline switch shares the section, so its arrival is what says the panel has settled.
    expect(await screen.findByTestId('properties-panel-outline')).toBeInTheDocument()
    expect(screen.queryByTestId('properties-panel-vrm-export')).not.toBeInTheDocument()
    expect(screen.queryByTestId('properties-panel-utility')).not.toBeInTheDocument()
  })

  it('keeps the thumbnail from being changed while a save runs, so two saves never overwrite each other', () => {
    const { rerender } = render(<Harness isSaving />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-thumbnail')).toBeDisabled()
    rerender(<Harness />)
    expect(screen.getByTestId('properties-panel-thumbnail')).toBeEnabled()
  })

  it('shows the item size against its limit, thumbnail and rarity image included', () => {
    render(<Harness size={{ bytes: 3_093_299, limit: 3 * 1024 * 1024 }} />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-size')).toHaveTextContent('2.95 / 3.00 MB')
    expect(screen.getByTestId('properties-panel-size')).not.toHaveAttribute('data-exceeded')
    expect(screen.queryByTestId('properties-panel-size-error')).not.toBeInTheDocument()
  })

  it('says what counts when an item is over its size limit, and still lets an edit of its details be saved', async () => {
    render(<Harness size={{ bytes: 3_243_672, limit: 3 * 1024 * 1024 }} />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-size')).toHaveAttribute('data-exceeded', 'true')
    expect(screen.getByTestId('properties-panel-size-error')).toHaveTextContent(
      'Over the size limit (thumbnail included). Reduce its textures or geometry.'
    )
    await userEvent.type(screen.getByTestId('properties-panel-name'), ' 2')
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('keeps a name without characters from being saved', async () => {
    render(<Harness />, { wrapper: Providers })
    await userEvent.clear(screen.getByTestId('properties-panel-name'))
    expect(screen.getByTestId('properties-panel-save')).toBeDisabled()
    await userEvent.type(screen.getByTestId('properties-panel-name'), 'a:b')
    expect(screen.getByTestId('properties-panel-name-error')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-save')).toBeDisabled()
  })

  it('hands a file change to the editor to save on its own, from the saved item', async () => {
    fileChange.result = {
      update: { item: { ...item, contents: { 'male/cap.glb': 'bafycap' } }, blobs: {} },
      thumbnail: null,
      category: null
    }
    const onSaveFile = vi.fn().mockResolvedValue(undefined)
    render(<Harness onSaveFile={onSaveFile} />, { wrapper: Providers })
    // Unsaved panel edits are not part of what the modal saves.
    await userEvent.type(screen.getByTestId('properties-panel-name'), ' 2')
    await userEvent.click(screen.getByTestId('properties-panel-actions'))
    await userEvent.click(screen.getByTestId('properties-panel-change-file'))
    expect(screen.getByTestId('mock-change-file-apply')).toHaveAttribute('data-shapes', '1')
    await userEvent.click(screen.getByTestId('mock-change-file-apply'))
    expect(onSaveFile).toHaveBeenCalledWith(fileChange.result)
    expect(screen.getByTestId('properties-panel-name')).toHaveValue('Hat 2')
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('keeps the file actions out of reach while a save is in flight', async () => {
    render(<Harness isSaving />, { wrapper: Providers })
    await userEvent.click(screen.getByTestId('properties-panel-actions'))
    expect(screen.getByTestId('properties-panel-change-file')).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByTestId('properties-panel-add-representation')).toHaveAttribute('aria-disabled', 'true')
  })

  it('keeps saving a stored item whose category the editor does not offer', async () => {
    render(<Harness subject={{ ...item, data: { ...item.data, category: 'body_shape' } }} />, { wrapper: Providers })
    await userEvent.type(screen.getByTestId('properties-panel-name'), ' 2')
    expect(screen.queryByTestId('properties-panel-category-error')).not.toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('keeps a description with a colon from being saved', async () => {
    render(<Harness />, { wrapper: Providers })
    await userEvent.type(screen.getByTestId('properties-panel-description'), ': red')
    expect(screen.getByTestId('properties-panel-description-error')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-save')).toBeDisabled()
  })

  it('locks every control and hides the footer when not editable', () => {
    render(<Harness editable={false} />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-readonly')).toBeInTheDocument()
    expect(screen.getByTestId('properties-panel-name')).toBeDisabled()
    expect(screen.getAllByTestId('properties-panel-hides-categories-pill').map(pill => pill.textContent)).toEqual([
      'Hair',
      'Helmet'
    ])
    expect(screen.queryByTestId('properties-panel-footer')).not.toBeInTheDocument()
  })

  it('offers the utility field for emotes too', async () => {
    const emote: Item = {
      ...item,
      type: ItemType.EMOTE,
      data: { category: 'fun', tags: [], loop: false, representations: item.data.representations }
    }
    render(<Harness subject={emote} />, { wrapper: Providers })
    await userEvent.type(await screen.findByTestId('properties-panel-utility'), 'greets')
    expect(screen.getByTestId('properties-panel-utility')).toHaveValue('greets')
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('tells a locked collection apart from missing edit rights in the read-only note', () => {
    const { unmount } = render(<Harness editable={false} />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-readonly')).toHaveTextContent(
      'only the collection owner and collaborators'
    )
    unmount()
    render(<Harness editable={false} readOnlyReason="lock" />, { wrapper: Providers })
    expect(screen.getByTestId('properties-panel-readonly')).toHaveTextContent('while the collection is being published')
  })

  it('lists tags read-only without the field when not editable', () => {
    render(<Harness editable={false} />, { wrapper: Providers })
    expect(screen.getAllByTestId('tags-input-tag').map(tag => tag.textContent)).toEqual(['cool'])
    expect(screen.queryByTestId('tags-input-field')).not.toBeInTheDocument()
    expect(screen.queryByTestId('tags-input-remove-cool')).not.toBeInTheDocument()
  })

  it('adds and removes tags', async () => {
    render(<Harness />, { wrapper: Providers })
    await userEvent.type(screen.getByTestId('tags-input-field'), 'pirate{enter}')
    expect(screen.getAllByTestId('tags-input-tag').map(tag => tag.textContent)).toEqual(['cool', 'pirate'])
    await userEvent.click(screen.getByTestId('tags-input-remove-cool'))
    expect(screen.getAllByTestId('tags-input-tag')).toHaveLength(1)
  })

  it('adds the campaign tag from the hint and stops offering it once present', async () => {
    campaign.current = { name: 'Summer Fest', mainTag: 'summer' }
    render(<Harness />, { wrapper: Providers })
    const hint = screen.getByTestId('properties-panel-campaign-hint')
    expect(hint).toHaveTextContent('Add the summer tag to enter this item in Summer Fest.')
    const tagButton = screen.getByTestId('properties-panel-campaign-tag')
    await userEvent.hover(tagButton)
    expect(await screen.findByRole('tooltip')).toHaveTextContent('Click to add')
    await userEvent.click(tagButton)
    expect(screen.getAllByTestId('tags-input-tag').map(tag => tag.textContent)).toEqual(['cool', 'summer'])
    expect(screen.queryByTestId('properties-panel-campaign-tag')).not.toBeInTheDocument()
    expect(hint).toHaveTextContent('Add the summer tag to enter this item in Summer Fest.')
    expect(within(hint).getByText('summer').tagName).toBe('STRONG')
    expect(screen.getByTestId('tags-input-field')).toHaveFocus()
    expect(screen.getByTestId('properties-panel-save')).toBeEnabled()
  })

  it('keeps the campaign hint out of the read-only panel', () => {
    campaign.current = { name: 'Summer Fest', mainTag: 'summer' }
    render(<Harness editable={false} />, { wrapper: Providers })
    expect(screen.queryByTestId('properties-panel-campaign-hint')).not.toBeInTheDocument()
  })
})
