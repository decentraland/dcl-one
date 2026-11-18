import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { ErrorBoundary } from './ErrorBoundary'

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

function Boom(): never {
  throw new Error('boom')
}

function renderBoundary(children: React.ReactNode) {
  return render(
    <TranslationProvider>
      <ErrorBoundary>{children}</ErrorBoundary>
    </TranslationProvider>
  )
}

beforeEach(() => {
  // Silence React's expected error logging for the throwing child.
  vi.spyOn(console, 'error').mockImplementation(() => {})
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('ErrorBoundary', () => {
  it('renders its children when nothing throws', () => {
    renderBoundary(<div data-testid="content" />)
    expect(screen.getByTestId('content')).toBeInTheDocument()
    expect(screen.queryByTestId('error-boundary')).not.toBeInTheDocument()
    expect(track).not.toHaveBeenCalled()
  })

  it('reports the crash screen under the same event name as the legacy builder', () => {
    renderBoundary(<Boom />)
    expect(track).toHaveBeenCalledWith('Error page')
  })

  it('shows the fallback with a reload CTA when a child throws', async () => {
    const reload = vi.fn()
    // jsdom doesn't implement navigation, so location is replaced wholesale.
    const originalLocation = window.location
    Object.defineProperty(window, 'location', {
      value: { ...originalLocation, reload },
      writable: true,
      configurable: true
    })

    renderBoundary(<Boom />)
    expect(screen.getByTestId('error-boundary')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('error-boundary-retry'))
    expect(reload).toHaveBeenCalled()

    Object.defineProperty(window, 'location', { value: originalLocation, writable: true, configurable: true })
  })
})
