import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BackpackPage } from '../../features/backpack/BackpackPage'
import { SessionProvider } from '../../features/session/SessionContext'
import { fakeProfileState, fakeSession } from '../harness'

function renderBackpack(initialTab?: 'emotes', modal = false) {
  const s = fakeSession()
  const emotes = { ...s.emotes, list: [{ urn: 'urn:wave', name: 'Wave' }], equip: vi.fn() }
  render(
    <SessionProvider value={s}>
      <BackpackPage backpack={{ ...s.backpack, open: true }} emotes={emotes} profile={fakeProfileState()} onNavigate={vi.fn()} setEngineViewport={vi.fn()} initialTab={initialTab} modal={modal} />
    </SessionProvider>
  )
  return emotes
}

describe('backpack hints', () => {
  it('the controls button toggles the hints and an outside click closes them', async () => {
    renderBackpack()
    const button = screen.getByRole('button', { name: 'Controls' })
    await userEvent.click(button)
    expect(screen.getByText('Left-click and drag avatar to rotate.')).toBeInTheDocument()
    expect(screen.getByText(/view their info/)).toBeInTheDocument()
    await userEvent.click(document.body)
    expect(screen.queryByText('Left-click and drag avatar to rotate.')).toBeNull()
  })

  it('in the modal (the lobby) the hints promise no item info on a click', async () => {
    renderBackpack(undefined, true)
    await userEvent.click(screen.getByRole('button', { name: 'Controls' }))
    expect(screen.getByText('Double-click items to equip.')).toBeInTheDocument()
    expect(screen.getByText('Left-click and drag avatar to rotate.')).toBeInTheDocument()
    expect(screen.getByText(/press the numeral key of the slot/)).toBeInTheDocument()
    expect(screen.queryByText(/view their info/)).toBeNull()
  })

  it('pressing a number key over an emote puts it in that wheel slot', () => {
    const emotes = renderBackpack('emotes')
    fireEvent.mouseEnter(screen.getByRole('button', { name: 'Wave' }))
    fireEvent.keyDown(window, { key: '3' })
    expect(emotes.equip).toHaveBeenCalledWith(3, 'urn:wave')
  })

  it('number keys do nothing when no emote is hovered', () => {
    const emotes = renderBackpack('emotes')
    const card = screen.getByRole('button', { name: 'Wave' })
    fireEvent.mouseEnter(card)
    fireEvent.mouseLeave(card)
    fireEvent.keyDown(window, { key: '3' })
    expect(emotes.equip).not.toHaveBeenCalled()
  })
})
