import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { CategorySelect } from './CategorySelect'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('CategorySelect', () => {
  it('shows a placeholder until a category is picked, then the category with its icon', async () => {
    const onChange = vi.fn()
    const { rerender } = render(
      <CategorySelect value={null} categories={['hat', 'upper_body']} onChange={onChange} />,
      { wrapper }
    )
    expect(screen.getByTestId('category-select')).toHaveTextContent('Select Category')

    await userEvent.click(screen.getByTestId('category-select'))
    expect(screen.getAllByRole('option')).toHaveLength(2)
    expect(screen.getByRole('img', { name: 'Upper Body' })).toBeInTheDocument()
    await userEvent.click(screen.getByTestId('category-select-option-upper_body'))
    expect(onChange).toHaveBeenCalledWith('upper_body')

    rerender(<CategorySelect value="upper_body" categories={['hat', 'upper_body']} onChange={onChange} />)
    expect(screen.getByTestId('category-select')).toHaveTextContent('Upper Body')
    expect(screen.getByRole('img', { name: 'Upper Body' })).toBeInTheDocument()
  })
})
