import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { Pagination } from './Pagination'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('Pagination', () => {
  it('marks the current page and navigates from numbers and chevrons', async () => {
    const onPageChange = vi.fn()
    render(<Pagination page={5} pages={9} onPageChange={onPageChange} />, { wrapper })

    expect(screen.getByRole('button', { name: '5' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('button', { name: '3' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '7' })).toBeInTheDocument()

    await userEvent.click(screen.getByRole('button', { name: '6' }))
    await userEvent.click(screen.getByRole('button', { name: 'Previous page' }))
    await userEvent.click(screen.getByRole('button', { name: 'Next page' }))
    expect(onPageChange.mock.calls).toEqual([[6], [4], [6]])
  })

  it('disables the chevrons at the ends', () => {
    const onPageChange = vi.fn()
    const view = render(<Pagination page={1} pages={3} onPageChange={onPageChange} />, { wrapper })
    expect(screen.getByRole('button', { name: 'Previous page' })).toBeDisabled()
    view.rerender(<Pagination page={3} pages={3} onPageChange={onPageChange} />)
    expect(screen.getByRole('button', { name: 'Next page' })).toBeDisabled()
  })
})
