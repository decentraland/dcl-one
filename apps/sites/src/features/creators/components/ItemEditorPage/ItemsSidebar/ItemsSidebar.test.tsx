import { describe, expect, it, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { BodyShape } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { ItemType, type Item } from '~/lib/items'
import { ItemsSidebar } from './ItemsSidebar'
import { type ComponentProps, type ReactNode } from 'react'

const collection: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: '0xabc',
  urn: 'urn:c1',
  isPublished: false,
  isApproved: false,
  itemCount: 2,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}

const make = (id: string, type: ItemType, bodyShapes: BodyShape[] = [BodyShape.MALE, BodyShape.FEMALE]): Item => ({
  id,
  name: id,
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '0xabc',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  type,
  data: { representations: [{ bodyShapes, mainFile: 'm.glb', contents: [] }] },
  contents: {},
  createdAt: 1,
  updatedAt: 1
})
const hat = make('hat', ItemType.WEARABLE)
const femaleOnly = make('dress', ItemType.WEARABLE, [BodyShape.FEMALE])
const dance = make('dance', ItemType.EMOTE)

/** Shows where the router ended up, so a test can tell a blocked link from one that navigated. */
function Location() {
  return <span data-testid="location">{useLocation().pathname}</span>
}

const wrapper = ({ children }: { children: ReactNode }) => (
  <MemoryRouter>
    <TranslationProvider>{children}</TranslationProvider>
    <Location />
  </MemoryRouter>
)

function renderSidebar(overrides: Partial<ComponentProps<typeof ItemsSidebar>> = {}) {
  const props: ComponentProps<typeof ItemsSidebar> = {
    collection,
    items: [hat, femaleOnly, dance],
    isLoading: false,
    selectedId: 'hat',
    dressedIds: ['hat'],
    bodyShape: BodyShape.MALE,
    isPlaying: false,
    mode: 'edit',
    back: { to: '/collections/c1', label: 'Back to collection' },
    collapsed: false,
    onToggleCollapsed: vi.fn(),
    canAddItems: true,
    onAddItems: vi.fn(),
    onSelect: vi.fn(),
    onToggleDressed: vi.fn(),
    onToggleEmotePlay: vi.fn(),
    ...overrides
  }
  render(<ItemsSidebar {...props} />, { wrapper })
  return props
}

describe('ItemsSidebar', () => {
  it('groups wearables and emotes and tells selecting from dressing apart', async () => {
    const props = renderSidebar()
    expect(screen.getByTestId('items-sidebar-wearables')).toBeInTheDocument()
    expect(screen.getByTestId('items-sidebar-emotes')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('items-sidebar-select-dance'))
    expect(props.onSelect).toHaveBeenCalledWith(dance)
    await userEvent.click(screen.getByTestId('items-sidebar-dress-dance'))
    expect(props.onToggleDressed).toHaveBeenCalledWith(dance)
    expect(props.onSelect).toHaveBeenCalledTimes(1)
  })

  it('dims items without a version for the body shape and blocks dressing them', () => {
    renderSidebar()
    expect(screen.getByTestId('items-sidebar-row-dress')).toHaveAttribute('data-unavailable')
    expect(screen.getByTestId('items-sidebar-dress-dress')).toBeDisabled()
    expect(screen.getByTestId('items-sidebar-row-hat')).not.toHaveAttribute('data-unavailable')
  })

  it('flags wearables made for a single body shape', () => {
    renderSidebar()
    expect(screen.getByTestId('items-sidebar-shape-dress-trigger')).toBeInTheDocument()
    expect(screen.queryByTestId('items-sidebar-shape-hat-trigger')).not.toBeInTheDocument()
    expect(screen.queryByTestId('items-sidebar-shape-dance-trigger')).not.toBeInTheDocument()
  })

  it('toggles playback when the selected, dressed emote row is clicked again', async () => {
    const props = renderSidebar({ selectedId: 'dance', dressedIds: ['dance'], isPlaying: true })
    expect(screen.getByTestId('items-sidebar-playing-dance')).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('items-sidebar-select-dance'))
    expect(props.onToggleEmotePlay).toHaveBeenCalledWith(dance)
    expect(props.onSelect).not.toHaveBeenCalled()
  })

  it('asks before the back link leaves and stays put when the answer is no', async () => {
    const onLeave = vi.fn().mockReturnValue(false)
    renderSidebar({ onLeave })
    await userEvent.click(screen.getByTestId('items-sidebar-back'))
    expect(onLeave).toHaveBeenCalledWith('/collections/c1')
    expect(screen.getByTestId('location')).toHaveTextContent('/')
    expect(screen.getByTestId('location')).not.toHaveTextContent('/collections/c1')
  })

  it('leaves for the collection when nothing holds the editor back', async () => {
    renderSidebar({ onLeave: vi.fn().mockReturnValue(true) })
    await userEvent.click(screen.getByTestId('items-sidebar-back'))
    expect(screen.getByTestId('location')).toHaveTextContent('/collections/c1')
  })

  it('shows no back link to a visitor with nowhere to go back to', () => {
    renderSidebar({ back: undefined })
    expect(screen.queryByTestId('items-sidebar-back')).not.toBeInTheDocument()
    expect(screen.getByTestId('items-sidebar-collection')).toHaveTextContent('Pirate Hats')
  })

  it('hides the header actions and the back link in review mode, where the review bar has them', () => {
    renderSidebar({ mode: 'review', onRename: vi.fn() })
    expect(screen.queryByTestId('items-sidebar-add-items')).not.toBeInTheDocument()
    expect(screen.queryByTestId('items-sidebar-rename')).not.toBeInTheDocument()
    expect(screen.queryByTestId('items-sidebar-back')).not.toBeInTheDocument()
    expect(screen.getByTestId('items-sidebar-collection')).toHaveTextContent('Pirate Hats')
  })

  it('offers renaming a draft collection from its title instead of linking away', async () => {
    const onRename = vi.fn()
    renderSidebar({ onRename })
    expect(screen.getByTestId('items-sidebar-collection').closest('a')).toBeNull()
    await userEvent.click(screen.getByTestId('items-sidebar-rename'))
    expect(onRename).toHaveBeenCalled()
  })

  it('collapses to thumbnails and icons while keeping every action reachable', async () => {
    const props = renderSidebar({ collapsed: true })
    expect(screen.getByTestId('items-sidebar')).toHaveAttribute('data-collapsed')
    // Name and status leave; the back link, the toggle, a square add button and the rows stay.
    expect(screen.getByTestId('items-sidebar-back')).toBeInTheDocument()
    expect(screen.getByTestId('items-sidebar-add-items')).toHaveAttribute('aria-label', 'Add items')
    expect(screen.getByTestId('items-sidebar-wearables-toggle')).toHaveAttribute('aria-label', 'Wearables')
    await userEvent.click(screen.getByTestId('items-sidebar-select-dance'))
    expect(props.onSelect).toHaveBeenCalledWith(dance)
    await userEvent.click(screen.getByTestId('items-sidebar-toggle'))
    expect(props.onToggleCollapsed).toHaveBeenCalled()
  })

  it('shows the empty copy for a collection without items', () => {
    renderSidebar({ items: [] })
    expect(screen.getByTestId('items-sidebar-empty')).toBeInTheDocument()
  })

  it('sets apart, in review, the items sent for review from the rest, marking new, changed and opened ones', () => {
    const changed = { ...make('cap', ItemType.WEARABLE), isApproved: true }
    renderSidebar({
      mode: 'review',
      items: [hat, changed, femaleOnly],
      review: { pendingIds: new Set(['hat', 'cap']), reviewedIds: new Set(['cap']) }
    })

    const toReview = screen.getByTestId('items-sidebar-to_review')
    expect(within(toReview).getByTestId('items-sidebar-tag-hat')).toHaveTextContent('New')
    expect(within(toReview).getByTestId('items-sidebar-tag-cap')).toHaveTextContent('Changes')
    expect(within(toReview).getByTestId('items-sidebar-reviewed-cap')).toBeInTheDocument()
    expect(within(toReview).queryByTestId('items-sidebar-reviewed-hat')).not.toBeInTheDocument()
    const rest = screen.getByTestId('items-sidebar-rest')
    expect(within(rest).getByTestId('items-sidebar-row-dress')).toBeInTheDocument()
    expect(within(rest).queryByTestId('items-sidebar-tag-dress')).not.toBeInTheDocument()
  })

  it('keeps nothing but the thumbnail in a collapsed row, so every thumbnail sits centered', () => {
    renderSidebar({ collapsed: true })
    const row = screen.getByTestId('items-sidebar-row-dress')
    expect(within(row).queryByTestId('items-sidebar-shape-dress-trigger')).not.toBeInTheDocument()
  })
})
