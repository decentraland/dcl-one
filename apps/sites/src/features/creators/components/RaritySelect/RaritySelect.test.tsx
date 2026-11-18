import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { RaritySelect } from './RaritySelect'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('RaritySelect', () => {
  it('shows the current rarity with its supply in brackets and lists every option with units on open', async () => {
    const onChange = vi.fn()
    render(<RaritySelect value="epic" onChange={onChange} />, { wrapper })
    expect(screen.getByTestId('rarity-select')).toHaveTextContent('Epic')
    expect(screen.getByTestId('rarity-select')).toHaveTextContent('Epic (1,000)')
    expect(screen.getByTestId('rarity-select')).not.toHaveTextContent('units')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()

    await userEvent.click(screen.getByTestId('rarity-select'))
    expect(screen.getAllByRole('option')).toHaveLength(8)
    expect(screen.getByTestId('rarity-select-option-unique')).toHaveTextContent('1 unit')
    expect(screen.getByTestId('rarity-select-option-common')).toHaveTextContent('100,000 units')
    expect(screen.getByTestId('rarity-select-option-epic')).toHaveAttribute('aria-selected', 'true')

    await userEvent.click(screen.getByTestId('rarity-select-option-legendary'))
    expect(onChange).toHaveBeenCalledWith('legendary')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })

  it('supports keyboard navigation and closes on Escape', async () => {
    const onChange = vi.fn()
    render(<RaritySelect value="epic" onChange={onChange} />, { wrapper })
    screen.getByTestId('rarity-select').focus()

    await userEvent.keyboard('{ArrowDown}')
    expect(screen.getByRole('listbox')).toBeInTheDocument()
    await userEvent.keyboard('{ArrowDown}{Enter}')
    expect(onChange).toHaveBeenCalledWith('rare')

    await userEvent.keyboard('{Enter}')
    expect(screen.getByRole('listbox')).toBeInTheDocument()
    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
  })
})
