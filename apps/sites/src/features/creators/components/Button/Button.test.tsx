import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Button } from './Button'

describe('Button', () => {
  it('renders its content and fires clicks', async () => {
    const onClick = vi.fn()
    render(<Button onClick={onClick}>Save</Button>)
    await userEvent.click(screen.getByRole('button', { name: 'Save' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('replaces content with a spinner and blocks clicks while loading', async () => {
    const onClick = vi.fn()
    render(
      <Button loading onClick={onClick}>
        Save
      </Button>
    )
    const button = screen.getByRole('button')
    expect(button).toBeDisabled()
    expect(screen.getByTestId('button-spinner')).toBeInTheDocument()
    expect(button).not.toHaveTextContent('Save')
    await userEvent.click(button)
    expect(onClick).not.toHaveBeenCalled()
  })

  it('renders as a link when asked to', () => {
    render(
      <Button as="a" href="https://example.com" variant="secondary">
        Learn more
      </Button>
    )
    expect(screen.getByRole('link', { name: 'Learn more' })).toHaveAttribute('href', 'https://example.com')
  })
})
