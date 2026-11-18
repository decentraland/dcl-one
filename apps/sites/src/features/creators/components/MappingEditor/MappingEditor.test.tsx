import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { MappingType, type Mapping } from '~/lib/mappings'
import { MappingEditor } from './MappingEditor'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderEditor(value: Mapping | null = { type: MappingType.ANY }) {
  const onChange = vi.fn()
  render(<MappingEditor value={value} onChange={onChange} />, { wrapper })
  return { onChange, lastMapping: () => onChange.mock.lastCall?.[0] as Mapping | null }
}

async function pickType(label: string) {
  await userEvent.click(screen.getByTestId('mapping-editor-type'))
  await userEvent.click(within(screen.getByRole('listbox')).getByText(label))
}

describe('MappingEditor', () => {
  it('reports a single token id once one is typed, and ignores anything but digits', async () => {
    const { lastMapping } = renderEditor()
    await pickType('Single')
    expect(lastMapping()).toBeNull()

    await userEvent.type(screen.getByTestId('mapping-editor-id'), '12a3')
    expect(screen.getByTestId('mapping-editor-id')).toHaveValue('123')
    expect(lastMapping()).toEqual({ type: MappingType.SINGLE, id: '123' })
  })

  it('reports a range only while it goes from low to high', async () => {
    const { lastMapping } = renderEditor()
    await pickType('Range')
    await userEvent.type(screen.getByTestId('mapping-editor-from'), '10')
    await userEvent.type(screen.getByTestId('mapping-editor-to'), '2')
    expect(lastMapping()).toBeNull()
    expect(screen.getByTestId('mapping-editor-error')).toBeInTheDocument()

    await userEvent.type(screen.getByTestId('mapping-editor-to'), '0')
    expect(lastMapping()).toEqual({ type: MappingType.RANGE, from: '10', to: '20' })
  })

  it('reads a list of ids separated by commas or spaces', async () => {
    const { lastMapping } = renderEditor()
    await pickType('Multiple')
    await userEvent.type(screen.getByTestId('mapping-editor-ids'), '1, 2 3')
    expect(lastMapping()).toEqual({ type: MappingType.MULTIPLE, ids: ['1', '2', '3'] })
  })

  it('follows a new saved value, but never over what the user is typing', async () => {
    const onChange = vi.fn()
    const { rerender } = render(<MappingEditor value={{ type: MappingType.SINGLE, id: '1' }} onChange={onChange} />, {
      wrapper
    })
    rerender(<MappingEditor value={{ type: MappingType.SINGLE, id: '2' }} onChange={onChange} />)
    expect(screen.getByTestId('mapping-editor-id')).toHaveValue('2')

    await userEvent.type(screen.getByTestId('mapping-editor-id'), '5')
    // The save of "2" comes back while the user has moved on to "25".
    rerender(<MappingEditor value={{ type: MappingType.SINGLE, id: '3' }} onChange={onChange} />)
    expect(screen.getByTestId('mapping-editor-id')).toHaveValue('25')
  })

  it('shows a mapping saved elsewhere for an item that had none', () => {
    const onChange = vi.fn()
    const { rerender } = render(<MappingEditor value={null} onChange={onChange} />, { wrapper })
    rerender(<MappingEditor value={{ type: MappingType.SINGLE, id: '7' }} onChange={onChange} />)
    expect(screen.getByTestId('mapping-editor-id')).toHaveValue('7')
  })

  it('starts from the saved mapping', () => {
    renderEditor({ type: MappingType.SINGLE, id: '42' })
    expect(screen.getByTestId('mapping-editor-id')).toHaveValue('42')
  })
})
