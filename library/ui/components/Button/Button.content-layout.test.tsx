import { render, screen, fireEvent } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { afterEach, beforeEach, describe, it, expect, vi, type Mock } from 'vitest'
import { Button } from './Button'
describe('when a caller chooses a supported Button content layout', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('and no layout is requested', () => {
    it('should default to intrinsic layout', () => {
      render(<Button>Default</Button>)
      expect(
        screen.getByRole('button').querySelector('[data-content-layout="intrinsic"]')
      ).not.toBeNull()
    })

    describe('and the button is re-rendered with the fill layout', () => {
      beforeEach(() => {
        const view = render(<Button>Default</Button>)
        view.rerender(<Button contentLayout="fill">Fill</Button>)
      })

      it('should switch to the fill layout', () => {
        expect(
          screen.getByRole('button').querySelector('[data-content-layout="fill"]')
        ).not.toBeNull()
      })

      it('should not leak the API prop into native HTML', () => {
        expect(screen.getByRole('button')).not.toHaveAttribute('contentLayout')
      })
    })
  })

  describe('and fill is requested for icon-only content', () => {
    it('should keep the content intrinsically centered', () => {
      render(
        <Button contentLayout="fill" iconOnly aria-label="Open">
          <span>Icon</span>
        </Button>
      )
      expect(
        screen.getByRole('button').querySelector('[data-content-layout="intrinsic"]')
      ).not.toBeNull()
    })
  })

  describe('and a fill button starts loading', () => {
    let action: Mock
    let node: HTMLElement

    beforeEach(() => {
      action = vi.fn()
      const view = render(
        <Button contentLayout="fill" onClick={action}>
          Save record
        </Button>
      )
      node = screen.getByRole('button')
      view.rerender(
        <Button contentLayout="fill" loading onClick={action}>
          Save record
        </Button>
      )
    })

    it('should keep the native button instead of replacing it', () => {
      expect(screen.getByRole('button')).toBe(node)
    })

    it('should preserve the loading name', () => {
      expect(node).toHaveAccessibleName('Save record')
    })

    it('should suppress repeat activation', () => {
      fireEvent.click(node)
      expect(action).not.toHaveBeenCalled()
    })
  })

  describe('and a fill link is clicked with a modifier', () => {
    let action: Mock
    let node: HTMLElement

    beforeEach(() => {
      action = vi.fn()
      render(
        <Button href="/destination" contentLayout="fill" onClick={action}>
          Destination
        </Button>
      )
      node = screen.getByRole('link')
    })

    it('should preserve the native link destination', () => {
      expect(node).toHaveAttribute('href', '/destination')
    })

    it('should leave the modifier click uncanceled and call the action once', () => {
      expect(fireEvent.click(node, { ctrlKey: true })).toBe(true)
      expect(action).toHaveBeenCalledOnce()
    })
  })

  describe.each(['', null, 12, 'other'])(
    'and the untyped presentation value is the invalid %j',
    (layout) => {
      let markup: string

      beforeEach(() => {
        markup = renderToString(<Button contentLayout={layout as never}>Action</Button>)
      })

      it('should preserve intrinsic layout', () => {
        expect(markup).toContain('data-content-layout="intrinsic"')
      })

      it('should not leak the API prop into the markup', () => {
        expect(markup).not.toContain('contentLayout=')
      })
    }
  )
})
