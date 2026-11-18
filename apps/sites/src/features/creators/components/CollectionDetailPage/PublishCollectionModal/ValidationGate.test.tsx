import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ComponentProps } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { type ItemSizes } from '~/hooks/useItemSize'
import { ItemType, type Item } from '~/lib/items'
import { ValidationSeverity, type ValidationIssue } from '~/lib/validation'
import { ValidationGate } from './ValidationGate'

const rerun = vi.fn()
vi.mock('~/hooks/useCollectionValidation', () => ({ useRerunItemValidation: () => rerun }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
const itemSizes: ItemSizes = { sizes: new Map(), unknown: new Set(), isMeasuring: false, remeasure: vi.fn() }
vi.mock('~/hooks/useItemSize', () => ({ useCatalystItemSizes: () => itemSizes }))

function makeItem(id: string): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xabc',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1
  }
}

const error: ValidationIssue = { code: 'skeleton', severity: ValidationSeverity.ERROR, message: 'Wrong skeleton' }
const warning: ValidationIssue = { code: 'textures', severity: ValidationSeverity.WARNING, message: 'Large texture' }
const broken = makeItem('broken')
const meh = makeItem('meh')
const clean = makeItem('clean')

type Props = ComponentProps<typeof ValidationGate>
const onPass = vi.fn()
const onClose = vi.fn()

function renderGate(overrides: Partial<Props> = {}) {
  const props: Props = {
    collectionId: 'c1',
    flow: 'publish',
    validation: { isValidating: true, results: [] },
    blockOnErrors: false,
    onPass,
    onClose,
    ...overrides
  }
  const view = render(<ValidationGate {...props} />, { wrapper: TranslationProvider })
  return {
    ...view,
    update: (next: Partial<Props>) => view.rerender(<ValidationGate {...props} {...next} />)
  }
}

beforeEach(() => {
  onPass.mockReset()
  onClose.mockReset()
  rerun.mockReset().mockResolvedValue([])
  vi.mocked(track).mockReset()
  itemSizes.sizes = new Map()
  itemSizes.unknown = new Set()
  itemSizes.isMeasuring = false
  itemSizes.remeasure = vi.fn().mockResolvedValue(undefined)
})

describe('ValidationGate', () => {
  it('waits for the checks and passes straight through when every item is clean', () => {
    const { update } = renderGate()
    expect(screen.getByTestId('publish-validating')).toBeInTheDocument()
    expect(onPass).not.toHaveBeenCalled()
    update({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    expect(onPass).toHaveBeenCalled()
  })

  it('waits for the items to be measured before deciding', () => {
    itemSizes.isMeasuring = true
    const { update } = renderGate({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    expect(screen.getByTestId('publish-validating')).toBeInTheDocument()
    itemSizes.isMeasuring = false
    update({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    expect(onPass).toHaveBeenCalled()
  })

  it('lists an item over its size limit and offers no way past it, even when errors do not block', () => {
    itemSizes.sizes = new Map([['meh', { bytes: 3_243_672, limit: 3 * 1024 * 1024 }]])
    renderGate({
      blockOnErrors: false,
      validation: {
        isValidating: false,
        results: [
          { item: meh, issues: [warning] },
          { item: clean, issues: [] }
        ]
      }
    })
    const card = screen.getByTestId('publish-validation-meh-card')
    expect(card).toHaveTextContent('This item is 3.09 MB, over the 3.00 MB limit')
    expect(screen.queryByTestId('publish-validation-clean-card')).not.toBeInTheDocument()
    expect(screen.queryByTestId('publish-validation-continue')).not.toBeInTheDocument()
    expect(track).toHaveBeenCalledWith(
      'Publish Validation Result',
      expect.objectContaining({ oversizedItems: 1, blocking: true })
    )
  })

  it('warns about an item whose size could not be measured, without blocking', async () => {
    itemSizes.unknown = new Set(['clean'])
    renderGate({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    expect(onPass).not.toHaveBeenCalled()
    expect(screen.getByTestId('publish-validation-clean-card')).toHaveTextContent("We couldn't check this item's size")
    await userEvent.click(screen.getByTestId('publish-validation-continue'))
    expect(onPass).toHaveBeenCalled()
  })

  it('measures an unmeasured item again when its checks are re-run', async () => {
    itemSizes.unknown = new Set(['clean'])
    renderGate({ validation: { isValidating: false, results: [{ item: clean, issues: [] }] } })
    await userEvent.click(screen.getByTestId('publish-validation-clean-rerun'))
    expect(itemSizes.remeasure).toHaveBeenCalledWith(clean)
  })

  it('reports a re-run against the rule book verdict alone, not the size issue', async () => {
    itemSizes.sizes = new Map([['meh', { bytes: 3_243_672, limit: 3 * 1024 * 1024 }]])
    renderGate({ validation: { isValidating: false, results: [{ item: meh, issues: [warning] }] } })
    await userEvent.click(screen.getByTestId('publish-validation-meh-rerun'))
    expect(rerun).toHaveBeenCalledWith(meh, 'publish', 'warnings')
  })

  it('closes from the waiting spinner', async () => {
    renderGate()
    await userEvent.click(screen.getByTestId('publish-validating-cancel'))
    expect(onClose).toHaveBeenCalled()
    expect(onPass).not.toHaveBeenCalled()
  })

  it('lists the flagged items, errors first, and offers no way past errors that block', async () => {
    renderGate({
      blockOnErrors: true,
      validation: {
        isValidating: false,
        results: [
          { item: meh, issues: [warning] },
          { item: clean, issues: [] },
          { item: broken, issues: [error] }
        ]
      }
    })
    const cards = screen.getAllByTestId(/^publish-validation-.*-card$/)
    expect(cards.map(card => card.dataset.testid)).toEqual([
      'publish-validation-broken-card',
      'publish-validation-meh-card'
    ])
    expect(screen.getByTestId('publish-validation-issues-title')).toHaveTextContent('Some items need your attention')
    expect(screen.queryByTestId('publish-validation-continue')).not.toBeInTheDocument()
    await userEvent.click(screen.getByTestId('publish-validation-back'))
    expect(onClose).toHaveBeenCalled()
    expect(onPass).not.toHaveBeenCalled()
  })

  it('lets the creator continue past errors when they do not block', async () => {
    renderGate({ validation: { isValidating: false, results: [{ item: broken, issues: [error] }] } })
    expect(screen.getByTestId('publish-validation-issues-title')).toHaveTextContent('Some items have issues')
    await userEvent.click(screen.getByTestId('publish-validation-continue'))
    expect(onPass).toHaveBeenCalled()
  })

  it('never blocks on a check that could not run, even when errors block', () => {
    const failed: ValidationIssue = { code: 'model', severity: ValidationSeverity.WARNING, message: 'Could not check' }
    renderGate({
      blockOnErrors: true,
      validation: { isValidating: false, results: [{ item: meh, issues: [failed] }] }
    })
    expect(screen.getByTestId('publish-validation-continue')).toBeInTheDocument()
  })

  it('lets the creator continue past warnings', async () => {
    renderGate({
      blockOnErrors: true,
      validation: { isValidating: false, results: [{ item: meh, issues: [warning] }] }
    })
    expect(screen.getByTestId('publish-validation-issues-title')).toHaveTextContent('A few things to review')
    await userEvent.click(screen.getByTestId('publish-validation-continue'))
    expect(onPass).toHaveBeenCalled()
  })

  it('follows a re-run live: a fixed item stays listed as passing and the last error unlocks Continue', async () => {
    const { update } = renderGate({
      flow: 'push_changes',
      blockOnErrors: true,
      validation: {
        isValidating: false,
        results: [
          { item: broken, issues: [error] },
          { item: meh, issues: [warning] }
        ]
      }
    })
    await userEvent.click(screen.getByTestId('publish-validation-broken-rerun'))
    expect(rerun).toHaveBeenCalledWith(broken, 'push_changes', 'errors')

    update({
      blockOnErrors: true,
      validation: {
        isValidating: false,
        results: [
          { item: broken, issues: [] },
          { item: meh, issues: [warning] }
        ]
      }
    })
    expect(screen.getByTestId('publish-validation-broken-pass')).toBeInTheDocument()
    expect(screen.getByTestId('publish-validation-issues-title')).toHaveTextContent('A few things to review')
    expect(screen.getByTestId('publish-validation-continue')).toBeInTheDocument()
    expect(onPass).not.toHaveBeenCalled()
  })

  it('speaks of sending for review rather than publishing in the push flow', () => {
    renderGate({
      flow: 'push_changes',
      blockOnErrors: true,
      validation: { isValidating: false, results: [{ item: broken, issues: [error] }] }
    })
    const content = screen.getByTestId('publish-validation-issues-content')
    expect(content).toHaveTextContent('before you can send your collection for review')
    expect(content).not.toHaveTextContent('publish')
  })

  it('tags every event with the flow it ran for', async () => {
    renderGate({
      flow: 'push_changes',
      validation: { isValidating: false, results: [{ item: meh, issues: [warning] }] }
    })
    await userEvent.click(screen.getByTestId('publish-validation-continue'))
    const events = vi
      .mocked(track)
      .mock.calls.filter(([name]) => name.startsWith('Publish Validation'))
      .map(([name, props]) => [name, (props as { flow: string }).flow])
    expect(events).toEqual([
      ['Publish Validation Started', 'push_changes'],
      ['Publish Validation Result', 'push_changes'],
      ['Publish Validation Resolved', 'push_changes']
    ])
  })
})
