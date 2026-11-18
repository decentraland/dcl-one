import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { CollectionNameInput } from './CollectionNameInput'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('CollectionNameInput', () => {
  it('reports typed values and counts characters against the limit', async () => {
    const onChange = vi.fn()
    render(<CollectionNameInput value="Hallo" onChange={onChange} testId="name" />, { wrapper })
    expect(screen.getByTestId('name-count')).toHaveTextContent('5/32')
    await userEvent.type(screen.getByTestId('name-input'), 'w')
    expect(onChange).toHaveBeenCalledWith('Hallow')
  })

  it('shows the uniqueness hint until an error replaces it', () => {
    const { rerender } = render(<CollectionNameInput value="" onChange={() => {}} testId="name" />, { wrapper })
    expect(screen.getByTestId('name-hint')).toHaveTextContent(/unique/i)
    expect(screen.queryByTestId('name-error')).not.toBeInTheDocument()
    rerender(<CollectionNameInput value="" onChange={() => {}} error="Taken" testId="name" />)
    expect(screen.getByTestId('name-error')).toHaveTextContent('Taken')
    expect(screen.queryByTestId('name-hint')).not.toBeInTheDocument()
  })
})
