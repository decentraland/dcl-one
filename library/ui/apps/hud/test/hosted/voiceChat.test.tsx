import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SessionProvider } from '../../features/session/SessionContext'
import { VoiceChatPanel } from '../../features/voiceChat/VoiceChatPanel'
import { SceneVoiceChatBanner } from '../../features/voiceChat/SceneVoiceChatBanner'
import { callGate, callingAlready, streamLive, streamPending } from '../../features/voiceChat/callGate'
import { streamTarget } from '../../features/notifications/summary'
import type { ActiveCommunityVoiceChat, AppNotification, VoiceChatParticipant, VoiceChatState } from '@explorer-core/engine/protocol'
import { resetProfileStore, seedProfiles } from '../../features/session/profileStore'
import { fakeSession } from '../harness'

const PEER = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'
const ME = '0x0000000000000000000000000000000000000001'

const idle: VoiceChatState = {
  status: 'disconnected',
  kind: 'none',
  callId: null,
  communityId: null,
  peer: null,
  connection: 'none',
  participants: [],
  micEnabled: false,
  localSpeaker: false,
  localRole: 'none',
  localRequestingToSpeak: false,
  error: null,
  errorIsNotice: false,
  errorSeq: 0
}

function participant(address: string, name: string, over: Partial<VoiceChatParticipant> = {}): VoiceChatParticipant {
  return { address, name, hasClaimedName: true, profilePictureUrl: '', role: 'user', isSpeaker: false, isRequestingToSpeak: false, isMuted: false, isSpeaking: false, ...over }
}

function renderPanel(state: Partial<VoiceChatState>, over: { active?: ActiveCommunityVoiceChat[]; scene?: ActiveCommunityVoiceChat | null; guest?: boolean } = {}) {
  const s = fakeSession()
  s.profile = { ...s.profile, data: { address: ME, name: 'Me', hasClaimedName: true, isGuest: over.guest === true } }
  s.voiceChat = { state: { ...idle, ...state }, active: over.active ?? [], activeLoaded: over.active != null, scene: over.scene ?? null, guest: over.guest === true, act: vi.fn() }
  render(
    <SessionProvider value={s}>
      <VoiceChatPanel />
      <SceneVoiceChatBanner />
    </SessionProvider>
  )
  return s
}

afterEach(() => resetProfileStore())

describe('voice call panel', () => {
  it('shows nothing while disconnected', () => {
    renderPanel({})
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('an incoming call names the caller and is accepted or refused', async () => {
    seedProfiles([{ address: PEER, name: 'Mojito' }])
    const s = renderPanel({ status: 'receivedCall', kind: 'private', callId: 'c1', peer: PEER })
    expect(screen.getByRole('dialog', { name: 'Voice call' })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent('Incoming Call from')
    expect(screen.getByRole('dialog', { name: 'Voice call' })).toHaveTextContent('Mojito')
    await userEvent.click(screen.getByRole('button', { name: 'Accept' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'acceptCall' })
    await userEvent.click(screen.getByRole('button', { name: 'Refuse' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'rejectCall' })
  })

  it('an outgoing call rings and can be hung up', async () => {
    const s = renderPanel({ status: 'startedCall', kind: 'private', callId: 'c1', peer: PEER })
    expect(screen.getByRole('status')).toHaveTextContent('Calling')
    await userEvent.click(screen.getByRole('button', { name: 'Hang up' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'endCall' })
  })

  it('in a connected call the mic toggles and the peer shows as speaking', async () => {
    const s = renderPanel({
      status: 'inCall',
      kind: 'private',
      callId: 'c1',
      peer: PEER,
      connection: 'connected',
      micEnabled: true,
      localSpeaker: true,
      participants: [participant(PEER, 'Mojito', { isSpeaker: true, isSpeaking: true })]
    })
    // the live region is the call's state; the talking line beside it is not re-announced
    expect(screen.getByRole('status')).toHaveTextContent(/^Ongoing Call$/)
    expect(screen.getByRole('dialog', { name: 'Voice call' })).toHaveTextContent('Mojito talking')
    const mic = screen.getByRole('button', { name: 'Mute microphone' })
    expect(mic).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(mic)
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'setMic', enabled: false })
  })

  it('busy and errors are shown and dismissed', async () => {
    const s = renderPanel({ status: 'busy', kind: 'private', peer: PEER, error: 'One of the users is busy calling someone else' })
    expect(screen.getByRole('alert')).toHaveTextContent('busy calling someone else')
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'dismissError' })
  })
})

describe('community stream panel', () => {
  const inStream = (over: Partial<VoiceChatState> = {}): Partial<VoiceChatState> => ({
    status: 'inCall',
    kind: 'community',
    communityId: 'c1',
    connection: 'connected',
    participants: [
      participant(ME, 'Me', { isSpeaker: true, role: 'owner' }),
      participant(PEER, 'Mojito', { isSpeaker: true, isSpeaking: true }),
      participant('0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4', 'Sharknado', { isRequestingToSpeak: true }),
      participant('0x1e105bb213754519903788022b962fe2b9c4b263', 'Pravus')
    ],
    ...over
  })

  it('lists the speakers as items whose buttons say their state', () => {
    renderPanel(inStream({ localRole: 'user', localSpeaker: false, participants: [participant(PEER, 'Mojito', { isSpeaker: true, isMuted: true })] }))
    const list = screen.getByRole('list', { name: 'Speakers' })
    expect(list.querySelectorAll(':scope > [role="listitem"] > button')).toHaveLength(1)
    expect(screen.getByRole('button', { name: 'Mojito, muted' })).toBeInTheDocument()
  })

  it('a listener raises a hand and leaves', async () => {
    const s = renderPanel(inStream({ localRole: 'user', localSpeaker: false }))
    expect(screen.getByRole('dialog', { name: 'Community stream' })).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent(/^Connected$/)
    expect(screen.getByRole('dialog', { name: 'Community stream' })).toHaveTextContent('4 · Mojito talking')
    await userEvent.click(screen.getByRole('button', { name: 'Raise hand' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'requestToSpeak', raise: true })
    expect(screen.queryByRole('button', { name: 'End stream' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Leave' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'leaveCall' })
  })

  it('a moderator sees the raised hands, approves one and can end the stream', async () => {
    const s = renderPanel(inStream({ localRole: 'owner', localSpeaker: true, micEnabled: false }))
    // the raised hands live in the People view, reached from the footer
    expect(screen.getByRole('button', { name: 'People' })).toHaveTextContent('1')
    await userEvent.click(screen.getByRole('button', { name: 'People' }))
    await userEvent.click(screen.getByRole('button', { name: 'Approve Sharknado' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'promoteSpeaker', address: '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4' })
    await userEvent.click(screen.getByRole('button', { name: 'Deny Sharknado' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'rejectSpeakRequest', address: '0x6723dcb07f3ca735223cd1c0acfa62dd994a1bb4' })
    await userEvent.click(screen.getByRole('button', { name: 'Unmute microphone' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'setMic', enabled: true })
    await userEvent.click(screen.getByRole('button', { name: 'End stream' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'endCommunityCall' })
  })

  it('a moderator demotes or removes a speaker from the People view', async () => {
    const s = renderPanel(inStream({ localRole: 'owner', localSpeaker: true }))
    await userEvent.click(screen.getByRole('button', { name: 'People' }))
    expect(screen.queryByRole('button', { name: 'Options for Me' })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: 'Options for Mojito' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Demote to listener' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'demoteSpeaker', address: PEER })
    await userEvent.click(screen.getByRole('button', { name: 'Options for Mojito' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Remove from stream' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'kickPlayer', address: PEER })
  })

  it('a speaker who is not a moderator can leave the stage', async () => {
    const s = renderPanel(inStream({ localRole: 'user', localSpeaker: true }))
    await userEvent.click(screen.getByRole('button', { name: 'Leave stage' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'demoteSpeaker', address: ME })
  })

  it('the listeners search narrows the list and the panel collapses', async () => {
    renderPanel(inStream({ localRole: 'user' }))
    expect(screen.queryByText('Pravus')).toBeNull() // the speakers grid only
    await userEvent.click(screen.getByRole('button', { name: 'People' }))
    expect(screen.getByText('Pravus')).toBeInTheDocument()
    await userEvent.type(screen.getByRole('textbox', { name: 'Search listeners' }), 'shark')
    expect(screen.queryByText('Pravus')).toBeNull()
    expect(screen.getByText('Sharknado')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Collapse stream panel' }))
    expect(screen.queryByRole('textbox', { name: 'Search listeners' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Leave' })).toBeInTheDocument()
  })
})

describe('the stream in this scene', () => {
  const here: ActiveCommunityVoiceChat = { communityId: 'c1', communityName: 'Decentraland Foundation', isMember: true, positions: ['0,0'], worlds: [], participantCount: 12, moderatorCount: 1 }

  it('offers to join, and goes away once joined', async () => {
    const s = fakeSession()
    s.voiceChat = { state: idle, active: [], activeLoaded: false, scene: here, guest: false, act: vi.fn() }
    const view = (session: typeof s) => (
      <SessionProvider value={session}>
        <VoiceChatPanel />
        <SceneVoiceChatBanner />
      </SessionProvider>
    )
    const { rerender } = render(view(s))
    expect(screen.getByRole('status', { name: 'Stream in this scene' })).toHaveTextContent('Decentraland Foundation')
    await userEvent.click(screen.getByRole('button', { name: 'Join stream' }))
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'joinCommunityCall', communityId: 'c1' })
    rerender(view({ ...s, voiceChat: { ...s.voiceChat, state: { ...idle, status: 'startingCall', kind: 'community', communityId: 'c1' } } }))
    expect(screen.queryByRole('status', { name: 'Stream in this scene' })).toBeNull()
  })

  it('is hidden while in that stream', () => {
    renderPanel({ status: 'inCall', kind: 'community', communityId: 'c1', connection: 'connected' }, { scene: here })
    expect(screen.queryByRole('status', { name: 'Stream in this scene' })).toBeNull()
  })
  it('is not offered to a guest', () => {
    renderPanel({}, { scene: here, guest: true })
    expect(screen.queryByRole('status', { name: 'Stream in this scene' })).toBeNull()
  })
})

describe('live streams', () => {
  const live: ActiveCommunityVoiceChat = { communityId: 'c1', communityName: 'C', isMember: true, positions: [], worlds: [], participantCount: 1, moderatorCount: 0 }

  it('the live list wins over a community snapshot once read', () => {
    expect(streamLive({ active: [], activeLoaded: false }, 'c1', true)).toBe(true)
    expect(streamLive({ active: [], activeLoaded: true }, 'c1', true)).toBe(false)
    expect(streamLive({ active: [live], activeLoaded: true }, 'c1', false)).toBe(true)
  })

  it('an old stream notification does not join a stream that has ended', () => {
    const n: AppNotification = { id: 'n', type: 'community_voice_chat_started', timestamp: '0', read: false, metadata: { communityId: 'c1' } }
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, active: [], activeLoaded: true, act: vi.fn() }
    streamTarget(n, s.voiceChat)?.()
    expect(vi.mocked(s.voiceChat.act)).not.toHaveBeenCalled()
    s.voiceChat = { ...s.voiceChat, active: [live] }
    streamTarget(n, s.voiceChat)?.()
    expect(vi.mocked(s.voiceChat.act)).toHaveBeenLastCalledWith({ op: 'joinCommunityCall', communityId: 'c1' })
  })

  it('a stream notification for the stream the user is in only opens its chat', () => {
    const n: AppNotification = { id: 'n', type: 'community_voice_chat_started', timestamp: '0', read: false, metadata: { communityId: 'c1' } }
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, state: { ...idle, status: 'inCall', kind: 'community', communityId: 'c1', localSpeaker: true }, active: [live], activeLoaded: true, act: vi.fn() }
    const openCommunity = vi.fn()
    streamTarget(n, s.voiceChat, { ...s.chat, communities: [], openCommunity })?.()
    expect(vi.mocked(s.voiceChat.act)).not.toHaveBeenCalled()
    expect(openCommunity).toHaveBeenCalledWith(expect.objectContaining({ id: 'c1' }))
  })

  it('a private call being placed does not hold a stream join back', () => {
    expect(streamPending({ ...idle, status: 'startingCall', kind: 'private' })).toBe(false)
    expect(streamPending({ ...idle, status: 'startingCall', kind: 'community', communityId: 'c1' })).toBe(true)
  })
})

describe('the call gate', () => {
  it('follows the partner state and the current call, as the call button tooltips do', () => {
    expect(callGate(null, { state: 'connected', online: true }, false).reason).toBe('Voice chat is not available.')
    expect(callGate(idle, { state: 'connected', online: true }, false)).toEqual({ allowed: true })
    expect(callGate(idle, { state: 'connected', online: false }, false).reason).toBe('This user is offline.')
    // the local user's own chat link, not the partner's
    expect(callGate(idle, { state: 'notConnected', online: true }, false).reason).toBe('You are not connected to chat.')
    expect(callGate(idle, { state: 'privateMessagesBlocked', online: true }, false).reason).toMatch(/friends/)
    expect(callGate(idle, { state: 'privateMessagesBlockedByOwnUser', online: true }, false).reason).toMatch(/privacy/)
    expect(callGate(idle, { state: 'blockedByOwnUser', online: true }, false).reason).toMatch(/Unblock/)
    expect(callGate({ ...idle, status: 'inCall', kind: 'private' }, { state: 'connected', online: true }, false).reason).toMatch(/already in a call/)
    expect(callGate({ ...idle, status: 'inCall', kind: 'community' }, { state: 'connected', online: true }, false).reason).toMatch(/community stream/)
    expect(callGate(idle, { state: 'connected', online: true }, true).reason).toMatch(/guest/)
  })

  it('a repeated call to the person being called is not refused', () => {
    const calling = { ...idle, kind: 'private', status: 'startedCall', peer: PEER } as const
    expect(callingAlready(calling, PEER.toUpperCase().replace('0X', '0x'))).toBe(true)
    expect(callingAlready({ ...calling, status: 'startingCall' }, PEER)).toBe(true)
    expect(callingAlready({ ...calling, status: 'inCall' }, PEER)).toBe(false)
    expect(callingAlready(calling, '0x1e105bb213754519903788022b962fe2b9c4b263')).toBe(false)
    expect(callingAlready({ ...calling, kind: 'community' }, PEER)).toBe(false)
  })
})
