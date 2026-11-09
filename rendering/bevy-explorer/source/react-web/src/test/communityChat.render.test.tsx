import { beforeEach, describe, it, expect, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Chat } from '../features/chat/Chat'
import type { ChatState, CommunityConversation } from '../features/session/useEngineSession'
import { pingModeFor, resetPingPrefs, setPingDefault, setPingMode } from '../features/chat/ping'
import type { ActiveCommunityVoiceChat, VoiceChatState } from '../engine/protocol'
import { SessionProvider } from '../features/session/SessionContext'
import { fakeSession } from './harness'

vi.mock('../features/communities/CommunityModal', () => ({ openCommunityModal: vi.fn(() => () => undefined) }))
import { openCommunityModal } from '../features/communities/CommunityModal'

// DOMAIN: community chat in the chat panel — the rail tab (badge, mention mark, close), the
// community title bar (online count, members list, the menu with View Community, Notification
// Ping, Delete chat history and Close conversation), and the input and reactions it keeps.
const MOJITO = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const community = (over: Partial<CommunityConversation> = {}): CommunityConversation => ({
  id: 'c1',
  key: 'community:c1',
  name: 'Decentraland Foundation',
  thumbnail: '',
  role: 'member',
  unread: 0,
  mention: false,
  members: null,
  online: 0,
  state: null,
  ...over
})
const chatWith = (over: Partial<ChatState>): ChatState => ({ ...fakeSession().chat, open: true, ...over })

describe('community chat panel', () => {
  beforeEach(() => {
    localStorage.clear()
    resetPingPrefs()
  })

  it('shows a rail tab per community with its unread count and mention mark, selectable and closable', async () => {
    const chat = chatWith({ communities: [community({ unread: 12, mention: true })], select: vi.fn(), closeCommunity: vi.fn() })
    render(<Chat chat={chat} />)
    expect(screen.getByRole('navigation', { name: 'Conversations' })).toBeInTheDocument()
    const tab = screen.getByRole('button', { name: 'Chat in Decentraland Foundation' })
    expect(tab).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getByText('9+')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'Mentions you' })).toBeInTheDocument()
    await userEvent.click(tab)
    expect(vi.mocked(chat.select)).toHaveBeenCalledWith('community:c1')
    await userEvent.click(screen.getByRole('button', { name: 'Close chat in Decentraland Foundation' }))
    expect(vi.mocked(chat.closeCommunity)).toHaveBeenCalledWith('c1')
  })

  it('a community channel shows its title bar, the online count opens the members list with roles', async () => {
    const members = [
      { address: MOJITO, name: 'Mojito', role: 'moderator' },
      { address: '0x3333333333333333333333333333333333333333', name: 'Cy', role: 'member' }
    ]
    const chat = chatWith({ channel: 'community:c1', communities: [community({ members, online: 2 })] })
    render(<Chat chat={chat} />)
    expect(screen.getByTitle('Decentraland Foundation')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /nearby$/ })).toBeNull()
    expect(screen.getByRole('textbox')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '2 online' }))
    expect(screen.getByRole('button', { name: 'View Mojito' })).toBeInTheDocument()
    expect(screen.getByText('Moderator')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'View Cy' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Back' }))
    expect(screen.queryByRole('button', { name: 'View Cy' })).toBeNull()
  })

  it('the menu views the community, sets the ping, deletes history or closes the chat', async () => {
    const chat = chatWith({ channel: 'community:c1', communities: [community()], deleteHistory: vi.fn(), closeCommunity: vi.fn() })
    render(<Chat chat={chat} />)
    const options = () => screen.getByRole('button', { name: 'Conversation options' })
    await userEvent.click(options())
    await userEvent.click(screen.getByText('View Community'))
    expect(vi.mocked(openCommunityModal)).toHaveBeenCalledWith('c1', expect.any(Function))
    await userEvent.click(options())
    await userEvent.click(screen.getByText('Notification Ping'))
    const mentions = screen.getByRole('menuitemradio', { name: 'Mentions Only' })
    // nothing set by hand: the default applies
    expect(screen.getByRole('menuitemradio', { name: 'Default' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('menuitemradio', { name: 'All Messages' })).toHaveAttribute('aria-checked', 'false')
    await userEvent.click(mentions)
    expect(pingModeFor('community:c1')).toBe('mentions')
    expect(screen.queryByRole('menu')).toBeNull()
    // back to the default from the menu, whatever the default is
    act(() => setPingDefault('none'))
    await userEvent.click(options())
    await userEvent.click(screen.getByText('Notification Ping'))
    expect(screen.getByRole('menuitemradio', { name: 'Mentions Only' })).toHaveAttribute('aria-checked', 'true')
    await userEvent.click(screen.getByRole('menuitemradio', { name: 'Default' }))
    expect(pingModeFor('community:c1')).toBe('none')
    await userEvent.click(options())
    await userEvent.click(screen.getByText('Delete chat history'))
    expect(vi.mocked(chat.deleteHistory)).toHaveBeenCalledWith('community:c1')
    await userEvent.click(options())
    await userEvent.click(screen.getByText('Close conversation'))
    expect(vi.mocked(chat.closeCommunity)).toHaveBeenCalledWith('c1')
  })

  it('the input gives way to the reason while a community message cannot go anywhere', () => {
    const { rerender } = render(<Chat chat={chatWith({ channel: 'community:c1', communities: [community({ state: 'noRouter' })] })} />)
    expect(screen.queryByRole('textbox')).toBeNull()
    expect(screen.getByRole('status')).toHaveTextContent('Community chat is not available right now.')
    rerender(<Chat chat={chatWith({ channel: 'community:c1', communities: [community({ state: 'notConnected' })] })} />)
    expect(screen.getByRole('status')).toHaveTextContent('You are not connected to chat.')
    rerender(<Chat chat={chatWith({ channel: 'community:c1', communities: [community({ state: 'connected' })] })} />)
    expect(screen.getByRole('textbox')).toBeInTheDocument()
  })

  it('closing a community from its title bar leaves its ping setting alone', async () => {
    setPingMode('community:c1', 'none')
    const chat = chatWith({ channel: 'community:c1', communities: [community()], closeCommunity: vi.fn() })
    render(<Chat chat={chat} />)
    await userEvent.click(screen.getByRole('button', { name: 'Conversation options' }))
    await userEvent.click(screen.getByText('Close conversation'))
    expect(vi.mocked(chat.closeCommunity)).toHaveBeenCalledWith('c1')
    expect(pingModeFor('community:c1')).toBe('none')
  })

  it('a community line can be reacted to', async () => {
    const chat = chatWith({
      channel: 'community:c1',
      communities: [community()],
      messages: [{ sender: MOJITO, message: 'hello all', channel: 'community:c1', messageId: `${MOJITO}:1`, id: 1, ts: Date.now() }],
      react: vi.fn()
    })
    render(<Chat chat={chat} />)
    expect(screen.getByText('hello all')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'React' }))
    await userEvent.click(screen.getByRole('button', { name: String.fromCodePoint(0x2665, 0xfe0f) }))
    expect(vi.mocked(chat.react)).toHaveBeenCalledWith(`${MOJITO}:1`, String.fromCodePoint(0x2665, 0xfe0f), false)
  })

  it('Nearby and DM title bars carry the ping menu too', async () => {
    render(<Chat chat={chatWith({})} />)
    await userEvent.click(screen.getByRole('button', { name: 'Conversation options' }))
    await userEvent.click(screen.getByText('Notification Ping'))
    await userEvent.click(screen.getByRole('menuitemradio', { name: 'None' }))
    expect(pingModeFor('Nearby')).toBe('none')
  })
})

describe('community chat and its voice stream', () => {
  const live = (over: Partial<ActiveCommunityVoiceChat> = {}): ActiveCommunityVoiceChat => ({
    communityId: 'c1',
    communityName: 'Decentraland Foundation',
    isMember: true,
    positions: [],
    worlds: [],
    participantCount: 12,
    moderatorCount: 1,
    ...over
  })
  const inStream = (communityId: string): VoiceChatState => ({ ...fakeSession().voiceChat.state!, status: 'inCall', kind: 'community', communityId, connection: 'connected' })
  function renderStream(chat: ChatState, voice: { active?: ActiveCommunityVoiceChat[]; state?: VoiceChatState; guest?: boolean } = {}) {
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, active: voice.active ?? [], state: voice.state ?? s.voiceChat.state, guest: voice.guest === true, act: vi.fn() }
    if (voice.guest) s.profile = { ...s.profile, data: { ...s.profile.data!, isGuest: true } }
    render(
      <SessionProvider value={s}>
        <Chat chat={chat} />
      </SessionProvider>
    )
    return s
  }

  beforeEach(() => {
    localStorage.clear()
  })

  it('marks the tab of a live community only', () => {
    const chat = chatWith({ communities: [community(), community({ id: 'c5', key: 'community:c5', name: 'Toxic Events' })] })
    renderStream(chat, { active: [live()] })
    expect(screen.getByRole('img', { name: 'Live voice stream' })).toBeInTheDocument()
    expect(screen.getAllByRole('img', { name: /voice stream/i })).toHaveLength(1)
    // the tab's label hides the mark inside it, so the mark describes the tab
    expect(screen.getByRole('button', { name: 'Chat in Decentraland Foundation' })).toHaveAccessibleDescription('Live voice stream')
    expect(screen.getByRole('button', { name: 'Chat in Toxic Events' })).not.toHaveAttribute('aria-describedby')
  })

  it('the tab of the stream the user is in shows the listening mark', () => {
    renderStream(chatWith({ communities: [community()] }), { active: [live()], state: inStream('c1') })
    expect(screen.getByRole('img', { name: 'In this voice stream' })).toBeInTheDocument()
    expect(screen.queryByRole('img', { name: 'Live voice stream' })).toBeNull()
  })

  it('no mark while the community does not stream', () => {
    renderStream(chatWith({ communities: [community()] }), { active: [live({ communityId: 'c9' })] })
    expect(screen.queryByRole('img', { name: /voice stream/i })).toBeNull()
  })

  it('the banner shows the live stream and its count, and joins it', async () => {
    const s = renderStream(chatWith({ channel: 'community:c1', communities: [community()] }), { active: [live()] })
    expect(screen.getByText('Live Voice Stream')).toBeInTheDocument()
    expect(screen.getByText('12')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: /Join the live voice stream/ }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenCalledWith({ op: 'joinCommunityCall', communityId: 'c1' })
  })

  it('the banner stays while the user is in another stream, and joining hands over to the engine', async () => {
    const s = renderStream(chatWith({ channel: 'community:c1', communities: [community()] }), { active: [live(), live({ communityId: 'c5' })], state: inStream('c5') })
    await userEvent.click(screen.getByRole('button', { name: /Join the live voice stream/ }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenCalledWith({ op: 'joinCommunityCall', communityId: 'c1' })
  })

  it('a guest is told voice chat is unavailable instead of joining', async () => {
    const s = renderStream(chatWith({ channel: 'community:c1', communities: [community()] }), { active: [live()], guest: true })
    await userEvent.click(screen.getByRole('button', { name: /Join the live voice stream/ }))
    expect(vi.mocked(s.voiceChat.act)).not.toHaveBeenCalled()
  })

  it.each([
    ['the community does not stream', [] as ActiveCommunityVoiceChat[], undefined],
    ['the user is in its stream', [live()], 'c1']
  ])('no banner while %s', (_, active, streaming) => {
    renderStream(chatWith({ channel: 'community:c1', communities: [community()] }), { active, state: streaming != null ? inStream(streaming) : undefined })
    expect(screen.queryByText('Live Voice Stream')).toBeNull()
  })

  it('no banner without a session (the chat on its own)', () => {
    render(<Chat chat={chatWith({ channel: 'community:c1', communities: [community()] })} />)
    expect(screen.queryByText('Live Voice Stream')).toBeNull()
  })
})
