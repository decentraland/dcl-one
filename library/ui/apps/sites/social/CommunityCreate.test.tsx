import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import CommunityCreate from './CommunityCreate'

const POLICY = 'https://example.org/content'
const NAMES = '/shop/names'

function setup(create = vi.fn().mockResolvedValue({ id: 'c1' }), onDone = vi.fn()) {
  render(
    <CommunityCreate
      create={create}
      onDone={onDone}
      contentPolicyUrl={POLICY}
      nameClaimUrl={NAMES}
    />
  )
  return { create, onDone }
}

const nameField = () => screen.getByRole('textbox', { name: /Community name/i })
const createButton = () => screen.getByRole('button', { name: 'Create' })

describe('CommunityCreate', () => {
  afterEach(() => {
    vi.resetAllMocks()
  })

  describe('when the form opens', () => {
    it('should name the dialog and link the content policy', () => {
      setup()
      expect(screen.getByRole('dialog', { name: 'Create a Community' })).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Content Policy' })).toHaveAttribute('href', POLICY)
      expect(createButton()).toHaveAttribute('aria-disabled', 'true')
    })
  })

  describe('when Create is pressed without a name', () => {
    it('should not send anything', async () => {
      const { create } = setup()
      await userEvent.type(nameField(), '   ')
      await userEvent.click(createButton())
      expect(create).not.toHaveBeenCalled()
    })
  })

  describe('when a named community is created with the defaults', () => {
    it('should send a public, listed community described by its name and report it created', async () => {
      const { create, onDone } = setup()
      await userEvent.type(nameField(), '  Builders  ')
      await userEvent.click(createButton())
      expect(create.mock.calls[0]?.[0]).toEqual({
        name: 'Builders',
        description: 'Builders',
        privacy: 'public',
        visibility: 'all'
      })
      expect(onDone).toHaveBeenCalledWith(true, { id: 'c1' })
    })
  })

  describe('when choosing membership', () => {
    it('should keep the explanation outside the options and update it after selection', async () => {
      setup()
      const user = userEvent.setup()
      const publicHint =
        'Anyone can become a member, view Community details, and join your Voice Streams.'
      expect(screen.getByText(publicHint)).toBeVisible()
      await user.click(screen.getByRole('combobox', { name: 'Membership' }))
      expect(screen.getByRole('option', { name: 'Public' })).toBeVisible()
      await user.click(screen.getByRole('option', { name: 'Private' }))
      expect(screen.queryByText(publicHint)).not.toBeInTheDocument()
      expect(
        screen.getByText('Members must be approved by an owner or moderator before they can join.')
      ).toBeVisible()
    })
  })

  describe('when the membership is private and the community is hidden', () => {
    it('should send a private, unlisted community with its description', async () => {
      const { create } = setup()
      await userEvent.type(nameField(), 'Builders')
      await userEvent.type(screen.getByRole('textbox', { name: 'Description' }), 'We build')
      await userEvent.click(screen.getByRole('combobox', { name: 'Membership' }))
      await userEvent.click(screen.getByRole('option', { name: /^Private/ }))
      await userEvent.click(
        screen.getByRole('switch', { name: 'Show the community in the directory' })
      )
      await userEvent.click(createButton())
      expect(create.mock.calls[0]?.[0]).toEqual({
        name: 'Builders',
        description: 'We build',
        privacy: 'private',
        visibility: 'unlisted'
      })
    })
  })

  describe('when the account has no NAME', () => {
    it('should show the NAME gate and return to the form from it', async () => {
      const { onDone } = setup(
        vi.fn().mockRejectedValue({ status: 401, message: "The user doesn't have any names" })
      )
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(
        screen.getByRole('dialog', { name: 'Get a NAME to Unlock Community Creation' })
      ).toBeInTheDocument()
      expect(onDone).not.toHaveBeenCalled()
      await userEvent.click(screen.getByRole('button', { name: 'Maybe later' }))
      expect(screen.getByRole('dialog', { name: 'Create a Community' })).toBeInTheDocument()
    })
  })

  describe('when the server refuses for another reason', () => {
    it('should announce its message and keep the form open', async () => {
      const { onDone } = setup(
        vi.fn().mockRejectedValue({ status: 401, message: 'Signature expired' })
      )
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(screen.getByRole('alert')).toHaveTextContent('Signature expired')
      expect(onDone).not.toHaveBeenCalled()
    })
  })

  describe('when the failure has no message', () => {
    it('should announce a generic failure', async () => {
      setup(vi.fn().mockRejectedValue(undefined))
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(screen.getByRole('alert')).toHaveTextContent("Couldn't create the community.")
    })
  })

  describe('while the community is being created', () => {
    it('should disable Cancel, refuse Escape and not send twice', async () => {
      let resolve: (value: unknown) => void = () => {}
      const create = vi.fn(() => new Promise((r) => (resolve = r)))
      const { onDone } = setup(create)
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(screen.getByRole('button', { name: 'Creating…' })).toHaveAttribute(
        'aria-disabled',
        'true'
      )
      expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled()
      fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
      await userEvent.click(screen.getByRole('button', { name: 'Creating…' }))
      expect(create).toHaveBeenCalledTimes(1)
      expect(onDone).not.toHaveBeenCalled()
      await act(async () => resolve({ id: 'c1' }))
      expect(onDone).toHaveBeenCalledWith(true, { id: 'c1' })
    })
  })

  describe('when the reader cancels', () => {
    it('should report nothing was created', async () => {
      const { create, onDone } = setup()
      await userEvent.click(screen.getByRole('button', { name: 'Cancel' }))
      expect(onDone).toHaveBeenCalledWith(false)
      expect(create).not.toHaveBeenCalled()
    })
  })

  describe('after the community is created', () => {
    it('should keep the form locked so a second press does not create it again', async () => {
      const { create } = setup()
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(createButton()).toHaveAttribute('aria-disabled', 'true')
      expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled()
      await userEvent.click(createButton())
      expect(create).toHaveBeenCalledTimes(1)
    })
  })

  describe('when the form starts from a kept draft', () => {
    it('should show the draft and send it as it stands', async () => {
      const create = vi.fn().mockResolvedValue({ id: 'c1' })
      render(
        <CommunityCreate
          create={create}
          initial={{
            name: 'Builders',
            description: 'We build',
            privacy: 'private',
            visibility: 'unlisted'
          }}
          contentPolicyUrl={POLICY}
          nameClaimUrl={NAMES}
        />
      )
      expect(nameField()).toHaveValue('Builders')
      expect(
        screen.getByRole('switch', { name: 'Show the community in the directory' })
      ).not.toBeChecked()
      await userEvent.click(createButton())
      expect(create.mock.calls[0]?.[0]).toEqual({
        name: 'Builders',
        description: 'We build',
        privacy: 'private',
        visibility: 'unlisted'
      })
    })
  })

  describe('when the NAME gate offers a NAME', () => {
    it('should link to the NAME page', async () => {
      setup(vi.fn().mockRejectedValue({ status: 401, message: "The user doesn't have any names" }))
      await userEvent.type(nameField(), 'Builders')
      await userEvent.click(createButton())
      expect(screen.getByRole('link', { name: 'Get a NAME' })).toHaveAttribute('href', NAMES)
    })
  })

  describe('when two forms are on the page', () => {
    it('should label each field with its own input', () => {
      setup()
      setup()
      const fields = screen.getAllByRole('textbox', { name: /Community name/i })
      expect(fields).toHaveLength(2)
      expect(fields[0]).not.toBe(fields[1])
    })
  })

  describe('when the description is left blank', () => {
    it('should send the name as the description and hand over the form as typed', async () => {
      const { create } = setup()
      await userEvent.type(nameField(), '  Builders ')
      await userEvent.click(createButton())
      expect(create).toHaveBeenCalledWith(
        { name: 'Builders', description: 'Builders', privacy: 'public', visibility: 'all' },
        { name: '  Builders ', description: '', privacy: 'public', visibility: 'all' }
      )
    })
  })
})
