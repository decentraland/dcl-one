import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MePanel, type MePanelProps } from '../../features/sidebar/panels/MePanel'
import { PopupHost, closeTopPopup, resetPopups } from '../../design/popups'

const ADDRESS = `0x10e${'0'.repeat(32)}a7a92`

// The panel the Me button opens: the player, the wallet address, the rows and what each does.
function panel(over: Partial<MePanelProps> = {}): MePanelProps {
  const props: MePanelProps = {
    profile: { address: ADDRESS, name: 'Tester', picture: undefined, hasClaimedName: false },
    backpackKey: 'I',
    onBackpack: vi.fn(),
    onProfile: vi.fn(),
    onClose: vi.fn(),
    ...over
  }
  render(
    <>
      <MePanel {...props} />
      <PopupHost />
    </>
  )
  return props
}

const signOutRow = (): HTMLElement => within(screen.getByRole('dialog', { name: 'Me' })).getByRole('button', { name: 'Sign Out' })
const question = (): HTMLElement | null => screen.queryByRole('dialog', { name: 'Sign out?' })
/** Press the Sign Out row and answer the question it asks with the button named `answer`. */
async function askToSignOut(answer?: string): Promise<MePanelProps> {
  const props = panel({ onSignOut: vi.fn(), onExit: vi.fn() })
  await userEvent.click(signOutRow())
  if (answer != null) await userEvent.click(within(question() as HTMLElement).getByRole('button', { name: answer }))
  return props
}

const rowNames = (): string[] => within(screen.getByRole('dialog', { name: 'Me' })).getAllByRole('button').map((b) => b.getAttribute('title') ?? b.textContent?.replace(/\[.*\]/, '') ?? '')

describe('Me panel', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    act(() => resetPopups())
  })

  it('is the dialog "Me" with the rows the bar gives an action for, in the design\'s order', () => {
    panel({ onBadges: vi.fn(), onSignOut: vi.fn(), onExit: vi.fn() })
    expect(rowNames()).toEqual(['Copy address', 'Backpack', 'Profile', 'Badges', 'Sign Out', 'Exit'])
    expect(screen.getAllByRole('separator')).toHaveLength(2)
  })

  it('draws no Badges, Sign Out or Exit row, and no second rule, without their actions', () => {
    panel()
    expect(rowNames()).toEqual(['Copy address', 'Backpack', 'Profile'])
    expect(screen.getAllByRole('separator')).toHaveLength(1)
  })

  it.each([
    ['Backpack', 'onBackpack'],
    ['Profile', 'onProfile'],
    ['Badges', 'onBadges'],
    ['Exit', 'onExit']
  ] as const)('%s runs its action once and closes the panel', async (name, prop) => {
    const props = panel({ onBadges: vi.fn(), onSignOut: vi.fn(), onExit: vi.fn() })
    await userEvent.click(screen.getByRole('button', { name }))
    expect(props[prop]).toHaveBeenCalledTimes(1)
    expect(props.onClose).toHaveBeenCalledTimes(1)
    for (const other of ['onBackpack', 'onProfile', 'onBadges', 'onSignOut', 'onExit'] as const) if (other !== prop) expect(props[other]).not.toHaveBeenCalled()
  })

  it('Sign Out asks first, in a dialog over the open panel, and signs nothing out yet', async () => {
    const props = await askToSignOut()
    const dialog = question() as HTMLElement
    expect(dialog).toHaveTextContent('You leave the world and return to the start screen.')
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: 'Sign Out' })).toBeInTheDocument()
    expect(props.onSignOut).not.toHaveBeenCalled()
    expect(props.onClose).not.toHaveBeenCalled()
  })

  it('Cancel closes the question without a sign-out, and the focus is back on the row', async () => {
    const props = await askToSignOut('Cancel')
    expect(question()).toBeNull()
    expect(props.onSignOut).not.toHaveBeenCalled()
    expect(props.onClose).not.toHaveBeenCalled()
    expect(signOutRow()).toHaveFocus()
  })

  it('the Cancel action (Escape) closes the question the same way', async () => {
    const props = await askToSignOut()
    act(() => closeTopPopup())
    expect(question()).toBeNull()
    expect(props.onSignOut).not.toHaveBeenCalled()
    expect(props.onClose).not.toHaveBeenCalled()
    expect(signOutRow()).toHaveFocus()
  })

  it('Sign Out in the question signs out once and closes the panel', async () => {
    const props = await askToSignOut('Sign Out')
    expect(question()).toBeNull()
    expect(props.onSignOut).toHaveBeenCalledTimes(1)
    expect(props.onClose).toHaveBeenCalledTimes(1)
    expect(props.onExit).not.toHaveBeenCalled()
  })

  it('names a row by its label alone: the key is a hint beside it', () => {
    panel()
    const backpack = screen.getByRole('button', { name: 'Backpack' })
    expect(backpack).toHaveTextContent('Backpack[I]')
    expect(screen.getByRole('button', { name: 'Profile' })).toHaveTextContent(/^Profile$/)
  })

  it('shows an unclaimed name with the end of its wallet as the tag', () => {
    panel()
    expect(screen.getByText('Tester')).toBeInTheDocument()
    expect(screen.getByText('#7a92')).toBeInTheDocument()
  })

  it('keeps a tag the name already carries', () => {
    panel({ profile: { address: ADDRESS, name: 'Tester#1234', hasClaimedName: false } })
    expect(screen.getByText('#1234')).toBeInTheDocument()
    expect(screen.queryByText('#7a92')).toBeNull()
  })

  it('gives a claimed name no tag', () => {
    panel({ profile: { address: ADDRESS, name: 'Tester', hasClaimedName: true } })
    expect(screen.queryByText(/^#/)).toBeNull()
  })

  it('copies the whole wallet address from its short form, says so, and stays open', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, { clipboard: { writeText } })
    const props = panel()
    const wallet = screen.getByTitle('Copy address')
    expect(wallet).toHaveTextContent('0x10e...a7a92')
    await userEvent.click(wallet)
    expect(writeText).toHaveBeenCalledWith(ADDRESS)
    expect(await screen.findByText('Copied')).toBeInTheDocument()
    expect(props.onClose).not.toHaveBeenCalled()
    await act(() => vi.advanceTimersByTimeAsync(1300))
    expect(wallet).toHaveTextContent('0x10e...a7a92')
    vi.useRealTimers()
  })

  it('leaves the head out until the profile has loaded, and keeps the rows', () => {
    panel({ profile: null })
    expect(screen.queryByTitle('Copy address')).toBeNull()
    expect(screen.queryByRole('separator')).toBeNull()
    expect(rowNames()).toEqual(['Backpack', 'Profile'])
  })

  it('offers no wallet button when the profile has no wallet address', () => {
    panel({ profile: { address: '', name: 'Tester', hasClaimedName: true } })
    expect(screen.queryByTitle('Copy address')).toBeNull()
    expect(screen.getByText('Tester')).toBeInTheDocument()
  })
})
