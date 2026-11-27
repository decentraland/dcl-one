import { act, cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CommunityMemberActions } from './CommunityMemberActions'

afterEach(cleanup)

describe('when an owner opens a member’s actions', () => {
  it.each([
    [false, 'Make moderator'],
    [true, 'Make member']
  ])(
    'should offer the other role (moderator now: %s) and removal',
    async (moderator, assignment) => {
      const user = userEvent.setup()
      render(
        <CommunityMemberActions
          name="Member B"
          moderator={moderator}
          canChangeRole
          onChange={vi.fn()}
        />
      )
      await user.click(screen.getByRole('button', { name: 'Member actions' }))
      expect(screen.getByRole('menuitem', { name: assignment })).toBeTruthy()
      expect(screen.getByRole('menuitem', { name: 'Remove member' })).toBeTruthy()
    }
  )

  it('should save the chosen role after the confirmation', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn(async () => {})
    render(
      <CommunityMemberActions name="Member B" moderator={false} canChangeRole onChange={onChange} />
    )
    await user.click(screen.getByRole('button', { name: 'Member actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Make moderator' }))
    await user.click(screen.getByRole('button', { name: 'Save role' }))
    expect(onChange).toHaveBeenCalledExactlyOnceWith('moderator')
    expect(screen.queryByRole('dialog')).toBeNull()
  })
})

describe('when a moderator removes a member', () => {
  it('should offer no role change', async () => {
    const user = userEvent.setup()
    render(
      <CommunityMemberActions
        name="Member B"
        moderator={false}
        canChangeRole={false}
        onChange={vi.fn()}
      />
    )
    await user.click(screen.getByRole('button', { name: 'Member actions' }))
    expect(screen.queryByRole('menuitem', { name: /^Make / })).toBeNull()
  })

  it('should send once, keep a refusal on screen and change nothing on cancel', async () => {
    const user = userEvent.setup()
    let fail!: (error: Error) => void
    const onChange = vi.fn(() => new Promise<void>((_, reject) => (fail = reject)))
    render(
      <CommunityMemberActions
        name="Member B"
        moderator={false}
        canChangeRole={false}
        onChange={onChange}
      />
    )
    await user.click(screen.getByRole('button', { name: 'Member actions' }))
    await user.click(screen.getByRole('menuitem', { name: 'Remove member' }))
    expect(onChange).not.toHaveBeenCalled()
    const dialog = screen.getByRole('dialog', { name: 'Remove member?' })
    await user.dblClick(within(dialog).getByRole('button', { name: 'Remove member' }))
    expect(onChange).toHaveBeenCalledExactlyOnceWith('remove')
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toBeDisabled()
    await act(async () => fail(new Error("You don't have permission to change this member.")))
    expect(within(dialog).getByRole('alert')).toHaveTextContent(
      "You don't have permission to change this member."
    )
    await user.click(within(dialog).getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(onChange).toHaveBeenCalledTimes(1)
  })
})
