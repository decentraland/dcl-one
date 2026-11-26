// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { ProfileMoreActions } from './ProfileMoreActions'

afterEach(cleanup)

type Actions = Parameters<typeof ProfileMoreActions>[0]['actions']

describe('ProfileMoreActions', () => {
  let user: ReturnType<typeof userEvent.setup>
  let actions: Actions

  beforeEach(() => {
    user = userEvent.setup()
  })

  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when it is operated from the keyboard', () => {
    beforeEach(() => {
      actions = [
        { label: 'Block', icon: null, onSelect: vi.fn() },
        { label: 'Unavailable', icon: null, disabled: true, onSelect: vi.fn() },
        { label: 'Share', icon: null, onSelect: vi.fn() }
      ]
      render(<ProfileMoreActions actions={actions} />)
    })

    it('should open from the keyboard, skip disabled items and return focus on Escape', async () => {
      const trigger = screen.getByRole('button', { name: 'More actions' })
      await user.tab()
      expect(document.activeElement).toBe(trigger)

      await user.keyboard('{ArrowDown}')
      expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Block' }))
      await user.keyboard('{ArrowDown}')
      expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Share' }))
      await user.keyboard('{ArrowDown}')
      expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Block' }))
      await user.keyboard('{ArrowUp}')
      expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Share' }))

      await user.keyboard('{Escape}')
      expect(screen.queryByRole('menu')).toBeNull()
      expect(document.activeElement).toBe(trigger)
    })
  })

  describe('when the trigger is clicked', () => {
    beforeEach(async () => {
      actions = [{ label: 'Share', icon: null, onSelect: vi.fn() }]
      render(<ProfileMoreActions actions={actions} />)
      await user.click(screen.getByRole('button', { name: 'More actions' }))
    })

    it('should open the menu', () => {
      expect(screen.getByRole('menu')).toBeTruthy()
    })

    describe('and the user presses outside the open menu', () => {
      beforeEach(async () => {
        await user.click(document.body)
      })

      it('should close the menu', () => {
        expect(screen.queryByRole('menu')).toBeNull()
      })
    })
  })
})
