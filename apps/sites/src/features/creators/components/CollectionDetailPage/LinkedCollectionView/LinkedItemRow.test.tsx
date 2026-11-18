import { describe, it, expect, vi, afterEach } from 'vitest'
import { type ReactNode } from 'react'
import { act, fireEvent, render, screen, within } from '@testing-library/react'
import { type ContractNetwork, type Mappings } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { ItemType, type Item } from '~/lib/items'
import { LinkedItemStatus } from '~/lib/linkedItemStatus'
import { LinkedItemRow, nextMappingStates } from './LinkedItemRow'

const CONTRACT = { network: 'amoy' as ContractNetwork, address: '0x1d9fb685c257e74f869ba302e260c0b68f5ebb37' }

const item: Item = {
  id: 'i1',
  name: 'Gold Hat',
  description: '',
  thumbnail: 'thumbnail.png',
  owner: '',
  urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats:gold-hat',
  isPublished: false,
  isApproved: false,
  inCatalyst: false,
  mappings: { amoy: { [CONTRACT.address]: [{ type: 'single', id: '1' }] } } as Mappings,
  type: ItemType.WEARABLE,
  data: { category: 'hat', representations: [] },
  contents: {},
  createdAt: 1,
  updatedAt: 1
}

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderRow(onSaveMappings = vi.fn().mockResolvedValue(item)) {
  const onMappingDirtyChange = vi.fn()
  const view = render(
    <LinkedItemRow
      item={item}
      contract={CONTRACT}
      status={LinkedItemStatus.READY_TO_PUBLISH}
      selectable
      selected={false}
      onSelectedChange={vi.fn()}
      editable
      onSaveMappings={onSaveMappings}
      onMappingDirtyChange={onMappingDirtyChange}
    />,
    { wrapper }
  )
  const input = within(screen.getByTestId('linked-item-mapping')).getByTestId('linked-item-mapping-editor-id')
  return { ...view, onSaveMappings, onMappingDirtyChange, input }
}

const expected = (id: string) => ({ amoy: { [CONTRACT.address]: [{ type: 'single', id }] } })

describe('LinkedItemRow', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows the URN identifier and saves a mapping change after a pause', () => {
    vi.useFakeTimers()
    const { onSaveMappings, input } = renderRow()
    expect(screen.getByTestId('linked-item-urn')).toHaveTextContent('gold-hat')

    fireEvent.change(input, { target: { value: '12' } })
    expect(onSaveMappings).not.toHaveBeenCalled()
    act(() => void vi.advanceTimersByTime(1000))

    expect(onSaveMappings).toHaveBeenCalledTimes(1)
    expect(onSaveMappings).toHaveBeenCalledWith(item, expected('12'))
  })

  it('saves a pending change when the row goes away before the pause ends', () => {
    vi.useFakeTimers()
    const { onSaveMappings, input, unmount } = renderRow()

    fireEvent.change(input, { target: { value: '7' } })
    unmount()

    expect(onSaveMappings).toHaveBeenCalledWith(item, expected('7'))
  })

  it('reports an edit as unsaved from the change until its save settles, so publishing can wait for it', async () => {
    vi.useFakeTimers()
    const { onMappingDirtyChange, input } = renderRow()

    fireEvent.change(input, { target: { value: '12' } })
    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', true, expect.any(Number))
    await act(async () => void vi.advanceTimersByTime(1000))

    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', false, expect.any(Number))
  })

  it('offers a retry after a failed save, which saves the same mapping again', async () => {
    vi.useFakeTimers()
    const onSave = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue(item)
    const { onMappingDirtyChange, input } = renderRow(onSave)

    fireEvent.change(input, { target: { value: '12' } })
    await act(async () => void vi.advanceTimersByTime(1000))
    await act(async () => void fireEvent.click(screen.getByTestId('linked-item-mapping-retry')))

    expect(onSave).toHaveBeenLastCalledWith(item, expected('12'))
    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', false, expect.any(Number))
    expect(screen.queryByTestId('linked-item-mapping-retry')).not.toBeInTheDocument()
  })

  it('keeps an edit unsaved when its save fails or the draft is invalid', async () => {
    vi.useFakeTimers()
    const { onMappingDirtyChange, input } = renderRow(vi.fn().mockRejectedValue(new Error('offline')))

    fireEvent.change(input, { target: { value: '12' } })
    await act(async () => void vi.advanceTimersByTime(1000))
    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', true, expect.any(Number))

    fireEvent.change(input, { target: { value: '' } })
    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', true, expect.any(Number))
  })

  it('keeps the row unsaved when its last edit fails to save on the way out', async () => {
    vi.useFakeTimers()
    const { onMappingDirtyChange, input, unmount } = renderRow(vi.fn().mockRejectedValue(new Error('offline')))

    fireEvent.change(input, { target: { value: '12' } })
    unmount()
    await act(async () => void (await Promise.resolve()))

    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', true, expect.any(Number))
  })

  it('reports the row clean when a save started before it went away lands afterwards', async () => {
    vi.useFakeTimers()
    let finish: () => void = () => undefined
    const onSave = vi.fn(() => new Promise<Item>(resolve => (finish = () => resolve(item))))
    const { onMappingDirtyChange, input, unmount } = renderRow(onSave)

    fireEvent.change(input, { target: { value: '12' } })
    act(() => void vi.advanceTimersByTime(1000))
    unmount()
    await act(async () => finish())

    expect(onMappingDirtyChange).toHaveBeenLastCalledWith('i1', false, expect.any(Number))
  })

  it('does not save an invalid or unchanged mapping', () => {
    vi.useFakeTimers()
    const { onSaveMappings, input } = renderRow()
    fireEvent.change(input, { target: { value: '' } })
    fireEvent.change(input, { target: { value: '1' } })
    act(() => void vi.advanceTimersByTime(2000))
    expect(onSaveMappings).not.toHaveBeenCalled()
  })
})

describe('nextMappingStates', () => {
  it('lets a newer row of the same item win over a late report from one already gone', () => {
    let states = nextMappingStates(new Map(), 'i1', true, 1)
    // The row is shown again and its new edit waits for the pause.
    states = nextMappingStates(states, 'i1', true, 2)
    // The first row's save lands late.
    states = nextMappingStates(states, 'i1', false, 1)
    expect(states.get('i1')).toEqual({ dirty: true, generation: 2 })
    states = nextMappingStates(states, 'i1', false, 2)
    expect(states.get('i1')).toEqual({ dirty: false, generation: 2 })
  })
})
