import '@testing-library/jest-dom/vitest'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { SwapLabel } from './SwapLabel'

describe('SwapLabel', () => {
  it('writes both texts as direct children of the button, in order', () => {
    render(
      <button type="button" aria-label="Cancel request">
        <SwapLabel idle="Request Sent" hover="Cancel Request" />
      </button>
    )
    const button = screen.getByRole('button', { name: 'Cancel request' })
    const [idle, hover] = Array.from(button.children)
    expect(button.children).toHaveLength(2)
    expect(idle).toHaveTextContent('Request Sent')
    expect(hover).toHaveTextContent('Cancel Request')
  })
})
