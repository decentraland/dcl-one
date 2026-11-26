// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import type { ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '@/data/lib/creators/intl'
import type { BoundRefusal } from '@/data/lib/creators/builder'
import { UploadErrorModal } from './UploadErrorModal'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

describe('UploadErrorModal', () => {
  let onRetry: Mock
  let refusal: BoundRefusal | null

  beforeEach(() => {
    onRetry = vi.fn()
    refusal = null
  })

  const renderModal = () =>
    render(<UploadErrorModal reason="generic" refusal={refusal} onCancel={vi.fn()} onRetry={onRetry} />, { wrapper })

  describe('when the node refused the upload at one of its bounds', () => {
    beforeEach(() => {
      refusal = { message: 'The upload is too large' }
    })

    it("should say the node's own reason instead of the generic copy", () => {
      renderModal()
      expect(screen.getByText('The upload is too large')).toBeInTheDocument()
    })

    it('should offer Try again', async () => {
      renderModal()
      await userEvent.click(screen.getByRole('button', { name: 'Try again' }))
      expect(onRetry).toHaveBeenCalledOnce()
    })

    describe('and it said how long to wait', () => {
      beforeEach(() => {
        refusal = { message: 'Too many file uploads of this wallet in progress', retryAfterSecs: 30 }
      })

      it('should tell the creator how long to wait before trying again', () => {
        renderModal()
        expect(
          screen.getByText('Too many file uploads of this wallet in progress. Try again in 30 seconds.')
        ).toBeInTheDocument()
      })
    })

    describe('and its words already say when to try again', () => {
      beforeEach(() => {
        refusal = { message: 'Too many requests, try again in a minute', retryAfterSecs: 30 }
      })

      it("should say the node's words alone", () => {
        renderModal()
        expect(screen.getByText('Too many requests, try again in a minute')).toBeInTheDocument()
      })
    })

    describe('and the wait is longer than a minute', () => {
      beforeEach(() => {
        refusal = { message: 'Too many file uploads of this wallet in progress', retryAfterSecs: 3600 }
      })

      it('should tell the wait in minutes', () => {
        renderModal()
        expect(
          screen.getByText('Too many file uploads of this wallet in progress. Try again in 60 minutes.')
        ).toBeInTheDocument()
      })
    })
  })

  describe('when the upload failed without a reason from the node', () => {
    it('should keep the generic copy', () => {
      renderModal()
      expect(screen.getByText('Something went wrong while uploading your items. Please try again.')).toBeInTheDocument()
    })
  })
})
