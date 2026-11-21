import { cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import ChromeBody from './ChromeBody'

afterEach(cleanup)

describe('ChromeBody', () => {
  it('links to a focusable main landmark', () => {
    render(<ChromeBody>Content</ChromeBody>)

    const main = screen.getByRole('main')
    expect(screen.getByRole('link', { name: 'Skip to content' })).toHaveAttribute('href', `#${main.id}`)
    expect(main).toHaveAttribute('tabindex', '-1')
  })

  it('keeps optional tools outside main and the footer inside it', () => {
    const { rerender } = render(
      <ChromeBody tools={<button>Tools</button>} footer={<a href="/terms">Terms</a>}>
        Content
      </ChromeBody>
    )

    const main = within(screen.getByRole('main'))
    expect(screen.getByRole('button', { name: 'Tools' })).toBeInTheDocument()
    expect(main.queryByRole('button', { name: 'Tools' })).toBeNull()
    expect(main.getByRole('link', { name: 'Terms' })).toBeInTheDocument()

    rerender(<ChromeBody footer={null}>Content</ChromeBody>)

    expect(screen.queryByRole('button', { name: 'Tools' })).toBeNull()
    expect(main.queryByRole('link', { name: 'Terms' })).toBeNull()
  })
})
