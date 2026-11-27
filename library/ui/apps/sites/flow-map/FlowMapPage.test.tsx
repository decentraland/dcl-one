// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import FlowMapPage from './FlowMapPage'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const ASCII = 'home -> /play'

function renderPage() {
  render(
    <FlowMapPage
      sections={[]}
      stats={{ routes: 1, states: 0, clicks: 1, loads: 0 }}
      ascii={ASCII}
      copy={{
        backHref: '/',
        backLabel: 'Back',
        crumb: 'Map',
        lede: 'The map.',
        asciiLabel: 'The map as text',
        honesty: 'Measured by hand.'
      }}
    />
  )
}

describe('FlowMapPage', () => {
  let user: ReturnType<typeof userEvent.setup>
  let writeText: ReturnType<typeof vi.fn<(text: string) => Promise<void>>>

  beforeEach(() => {
    user = userEvent.setup()
    writeText = vi.fn<(text: string) => Promise<void>>()
    vi.spyOn(navigator, 'clipboard', 'get').mockReturnValue({
      writeText
    } as unknown as Clipboard)
  })

  describe('when the text version is copied', () => {
    beforeEach(() => {
      writeText.mockResolvedValue(undefined)
      renderPage()
    })

    it('should copy the ascii source and say so on the button', async () => {
      const button = screen.getByRole('button', { name: 'Copy' })
      expect(button.getAttribute('type')).toBe('button')
      await user.click(button)
      expect(writeText).toHaveBeenCalledWith(ASCII)
      expect(await screen.findByRole('button', { name: 'Copied \u{2713}' })).toBeTruthy()
    })
  })

  describe('when the clipboard refuses the text', () => {
    beforeEach(() => {
      writeText.mockRejectedValue(new Error('denied'))
      renderPage()
    })

    it('should keep the button as it was', async () => {
      await user.click(screen.getByRole('button', { name: 'Copy' }))
      expect(writeText).toHaveBeenCalledOnce()
      expect(screen.getByRole('button', { name: 'Copy' })).toBeTruthy()
    })
  })
})
