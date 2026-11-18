import { render, screen, fireEvent } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { describe, it, expect, vi } from 'vitest'
import { Button } from './Button'
describe('when a caller chooses a supported Button content layout', () => {
  it('should default to intrinsic layout without leaking the API prop into native HTML', () => {
    const v = render(<Button>Default</Button>)
    expect(
      screen.getByRole('button').querySelector('[data-content-layout="intrinsic"]')
    ).not.toBeNull()
    v.rerender(<Button contentLayout="fill">Fill</Button>)
    expect(screen.getByRole('button').querySelector('[data-content-layout="fill"]')).not.toBeNull()
    expect(screen.getByRole('button')).not.toHaveAttribute('contentLayout')
  })
  it('should keep icon-only content intrinsically centered even when fill is requested', () => {
    render(
      <Button contentLayout="fill" iconOnly aria-label="Open">
        <span>Icon</span>
      </Button>
    )
    expect(
      screen.getByRole('button').querySelector('[data-content-layout="intrinsic"]')
    ).not.toBeNull()
  })
  it('should preserve loading names and suppress repeat activation without replacing the native button', () => {
    const action = vi.fn()
    const v = render(
        <Button contentLayout="fill" onClick={action}>
          Save record
        </Button>
      ),
      node = screen.getByRole('button')
    v.rerender(
      <Button contentLayout="fill" loading onClick={action}>
        Save record
      </Button>
    )
    expect(screen.getByRole('button')).toBe(node)
    expect(node).toHaveAccessibleName('Save record')
    fireEvent.click(node)
    expect(action).not.toHaveBeenCalled()
  })
  it('should preserve native link destinations and uncanceled modifier behavior', () => {
    const action = vi.fn()
    render(
      <Button href="/destination" contentLayout="fill" onClick={action}>
        Destination
      </Button>
    )
    const node = screen.getByRole('link')
    expect(node).toHaveAttribute('href', '/destination')
    expect(fireEvent.click(node, { ctrlKey: true })).toBe(true)
    expect(action).toHaveBeenCalledOnce()
  })
  it.each(['', null, 12, 'other'])(
    'should preserve intrinsic layout for an invalid untyped presentation value %j',
    (layout) => {
      const markup = renderToString(<Button contentLayout={layout as never}>Action</Button>)
      expect(markup).toContain('data-content-layout="intrinsic"')
      expect(markup).not.toContain('contentLayout=')
    }
  )
})
