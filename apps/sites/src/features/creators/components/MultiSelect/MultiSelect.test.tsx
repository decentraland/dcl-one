import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { MultiSelect } from './MultiSelect'

const options = [
  { value: 'to_review', label: 'To review' },
  { value: 'approved', label: 'Approved' },
  { value: 'rejected', label: 'Rejected' }
]

function renderSelect(values: string[], variant?: 'compact') {
  const onChange = vi.fn()
  render(
    <TranslationProvider>
      <MultiSelect
        values={values}
        options={options}
        onChange={onChange}
        placeholder="All statuses"
        ariaLabel="Status"
        variant={variant}
        testId="status"
      />
    </TranslationProvider>
  )
  return onChange
}

describe('MultiSelect', () => {
  it('lists every pick on the trigger, or the placeholder with none', () => {
    renderSelect(['to_review', 'approved'])
    expect(screen.getByTestId('status')).toHaveTextContent('To review, Approved')
  })

  it('counts the picks beyond the first on the compact trigger', () => {
    renderSelect(['to_review', 'approved', 'rejected'], 'compact')
    expect(screen.getByTestId('status')).toHaveTextContent('To review +2')
    expect(screen.getByTestId('status')).toHaveAttribute('aria-label', 'Status: To review +2')
  })

  it('shows the placeholder with nothing picked', () => {
    renderSelect([], 'compact')
    expect(screen.getByTestId('status')).toHaveTextContent('All statuses')
  })

  it('toggles a pick without closing the list', () => {
    const onChange = renderSelect(['to_review'])
    fireEvent.click(screen.getByTestId('status'))
    fireEvent.click(screen.getByTestId('status-option-approved'))
    expect(onChange).toHaveBeenCalledWith(['to_review', 'approved'])
    expect(screen.getByTestId('status-listbox')).toBeInTheDocument()
  })
})
