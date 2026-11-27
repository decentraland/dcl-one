// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { HelpStatus, type HelpStatusReport } from './HelpStatus'

afterEach(cleanup)

const REPORT: HelpStatusReport = {
  overall: 'slow',
  overallText: 'Some services are slow',
  services: [
    { name: 'Catalyst', state: 'ok' },
    { name: 'Marketplace', state: 'down' }
  ]
}

describe('HelpStatus', () => {
  let user: ReturnType<typeof userEvent.setup>

  beforeEach(() => {
    user = userEvent.setup()
  })

  describe('when the services are still being checked', () => {
    beforeEach(() => {
      render(
        <HelpStatus
          label="System Status"
          report={null}
          detailsLabel="Details"
          detailsHref="https://status.example/"
        />
      )
    })

    it('should show no toggle and announce nothing', () => {
      expect(screen.queryByRole('button', { name: 'System Status' })).toBeNull()
      expect(screen.getByRole('status').textContent).toBe('')
    })
  })

  describe('when the report is in', () => {
    let toggle: HTMLElement

    beforeEach(() => {
      render(
        <HelpStatus
          label="System Status"
          report={REPORT}
          detailsLabel="Details"
          detailsHref="https://status.example/"
        />
      )
      toggle = screen.getByRole('button', { name: 'System Status' })
    })

    it('should render a closed toggle that controls the service list', () => {
      expect(toggle.getAttribute('type')).toBe('button')
      expect(toggle.getAttribute('aria-expanded')).toBe('false')
      const list = document.getElementById(toggle.getAttribute('aria-controls')!)
      expect(list?.hidden).toBe(true)
    })

    it('should open the list on a click and close it on a second one', async () => {
      await user.click(toggle)
      expect(toggle.getAttribute('aria-expanded')).toBe('true')
      expect(screen.getByText('Marketplace')).toBeTruthy()
      await user.click(toggle)
      expect(toggle.getAttribute('aria-expanded')).toBe('false')
    })

    it('should close the list on Escape and return focus to the toggle', async () => {
      await user.click(toggle)
      screen.getByRole('link', { name: 'Details' }).focus()
      await user.keyboard('{Escape}')
      expect(toggle.getAttribute('aria-expanded')).toBe('false')
      expect(document.activeElement).toBe(toggle)
    })

    it('should close the list on a press outside it', async () => {
      await user.click(toggle)
      await user.click(document.body)
      expect(toggle.getAttribute('aria-expanded')).toBe('false')
    })
  })
})
