import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Panel } from './Panel'

describe('Panel', () => {
  it('is a plain container that keeps the caller role, name, class and content', () => {
    render(
      <Panel role="dialog" aria-label="Volume" className="mine">
        <span>content</span>
      </Panel>
    )
    const panel = screen.getByRole('dialog', { name: 'Volume' })
    expect(panel).toHaveClass('mine')
    expect(panel).toHaveTextContent('content')
    expect(panel).not.toHaveAttribute('data-blur')
    expect(panel).not.toHaveAttribute('blur')
  })

  it('marks the frosted surface', () => {
    render(<Panel blur data-testid="panel" />)
    expect(screen.getByTestId('panel')).toHaveAttribute('data-blur')
  })
})
