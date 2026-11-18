import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { NotFoundPage } from './NotFoundPage'

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

describe('NotFoundPage', () => {
  it('explains the miss, offers the way home and counts the view', () => {
    render(
      <TranslationProvider>
        <MemoryRouter initialEntries={['/nope']}>
          <NotFoundPage />
        </MemoryRouter>
      </TranslationProvider>
    )
    expect(screen.getByText('Page not found')).toBeInTheDocument()
    expect(screen.getByTestId('not-found-home')).toHaveAttribute('href', '/')
    expect(track).toHaveBeenCalledWith('Not found page')
  })
})
