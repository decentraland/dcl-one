import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { PublishErrorModal } from './PublishErrorModal'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('PublishErrorModal', () => {
  it('offers to try again on a generic failure and reassures about the credits', async () => {
    const onRetry = vi.fn()
    render(<PublishErrorModal reason="generic" onCancel={vi.fn()} onRetry={onRetry} />, { wrapper })
    expect(screen.getByTestId('publish-error-modal-description')).toHaveTextContent(/Credits are safe/)
    await userEvent.click(screen.getByTestId('publish-error-retry'))
    expect(onRetry).toHaveBeenCalled()
  })

  it('explains an insufficient credits balance', () => {
    render(<PublishErrorModal reason="insufficient_credits" onCancel={vi.fn()} onRetry={vi.fn()} />, { wrapper })
    expect(screen.getByTestId('publish-error-modal-description')).toHaveTextContent(/enough Credits/)
    expect(screen.getByTestId('publish-error-retry')).toBeInTheDocument()
  })

  it('asks to switch the wallet back to the signed-in account, and offers a retry', () => {
    render(<PublishErrorModal reason="wallet" onCancel={vi.fn()} onRetry={vi.fn()} />, { wrapper })
    expect(screen.getByTestId('publish-error-modal-description')).toHaveTextContent(/switch it back to the account/i)
    expect(screen.getByTestId('publish-error-retry')).toBeInTheDocument()
  })

  it('drops the retry action when retrying can never succeed', () => {
    render(<PublishErrorModal reason="locked" onCancel={vi.fn()} onRetry={vi.fn()} />, { wrapper })
    expect(screen.queryByTestId('publish-error-retry')).not.toBeInTheDocument()
  })
})
