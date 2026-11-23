// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CommunityError } from './CommunityError'

afterEach(cleanup)

describe('CommunityError', () => {
  let onRetry: ReturnType<typeof vi.fn>

  beforeEach(() => {
    onRetry = vi.fn()
  })

  describe('when the community does not exist', () => {
    beforeEach(() => {
      render(<CommunityError reason="not-found" onRetry={onRetry} />)
    })

    it('should say so and offer no Try again', () => {
      expect(screen.getByRole('heading', { level: 1, name: 'Community not found' })).toBeTruthy()
      expect(screen.getByRole('alert')).toBeTruthy()
      expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull()
    })
  })

  describe('when the community could not be read', () => {
    beforeEach(() => {
      render(<CommunityError reason="unavailable" onRetry={onRetry} />)
    })

    it('should say so', () => {
      expect(screen.getByRole('heading', { level: 1, name: 'Community unavailable' })).toBeTruthy()
    })

    describe('and the visitor clicks Try again', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
      })

      it('should read it again', () => {
        expect(onRetry).toHaveBeenCalledTimes(1)
      })
    })
  })
})
