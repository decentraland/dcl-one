// The voice chat panel: a private call (incoming, outgoing, connecting, in call, busy / error) or a
// community stream (speakers, people, controls), driven by the engine's state snapshots
// (session.voiceChat.state). It is stacked above the chat in their shared area, so it docks over
// the chat column and the chat shrinks by its height (VOICE_PANEL_SIZE, --voice-dock). The private
// call is one 46px titlebar; the community stream is a header, the SPEAKERS grid and a footer, with
// the People view in its place. Actions go to the engine, whose next snapshot
// moves the panel.
import { useEffect, useMemo, useRef, useState } from 'react'
import { Avatar, ContextMenu, ControlButton, Kebab, MaskIcon, SearchField, Tooltip, VerifiedBadge, type ContextMenuItem } from '../../design'
import type { VoiceChatParticipant, VoiceChatState } from '../../engine/protocol'
import { nameColor, splitName } from '../../lib/identity'
import { registerCancelLayer } from '../../lib/cancelLayers'
import { openProfileCard } from '../profileCard/ProfileCard'
import { knownUserColor, useProfile } from '../session/profileStore'
import { useSession } from '../session/SessionContext'
import { voiceActive } from './callGate'
import callIcon from '../../assets/voice/call.png'
import endCallIcon from '../../assets/voice/end-call.png'
import rejectCallIcon from '../../assets/voice/reject-call.png'
import micOnIcon from '../../assets/voice/mic-on.png'
import micOffIcon from '../../assets/voice/mic-off.png'
import raiseHandIcon from '../../assets/voice/raise-hand.png'
import handIcon from '../../assets/voice/hand.png'
import handNotificationIcon from '../../assets/voice/hand-notification.png'
import peopleIcon from '../../assets/voice/people.png'
import leaveStageIcon from '../../assets/voice/leave-stage.png'
import endStreamIcon from '../../assets/voice/end-stream.png'
import callErrorIcon from '../../assets/voice/call-error.png'
import acceptRequestIcon from '../../assets/voice/accept-request.png'
import rejectRequestIcon from '../../assets/voice/reject-request.png'
import speakingFeedbackIcon from '../../assets/voice/speaking-feedback.png'
import styles from './VoiceChatPanel.module.css'

/** The speakers shown in the grid. */
const MAX_VISIBLE_SPEAKERS = 8

/** The panel's height (1080p canvas px) by state. The chat
 *  column below gives up the same amount (Chat.module.css reads --voice-dock). */
export const VOICE_PANEL_SIZE = {
  /** A titlebar, or a collapsed stream: the chat reserves this much always. */
  bar: 46,
  /** An expanded stream with one line of speakers (up to maxSpeakersPerLine). */
  expanded1: 215,
  /** An expanded stream with two lines of speakers. */
  expanded2: 305,
  maxSpeakersPerLine: 4,
  /** The "stream in this scene" banner. */
  scene: 50
} as const

/** The chat's shared area (738px): the voice panel sits at its top, the chat fills the rest. */
const SHARED_AREA = 738
const PANEL_GAP = 3
/** The collapsed chat (its input row only). */
const CHAT_INPUT_ROW = 46

/** Publish the panel's height so the open chat column shrinks under it (Chat.module.css), and give
 *  the panel the chat's height to sit on. */
export function useVoiceDock(height: number | null): React.CSSProperties {
  const { chat, friends } = useSession()
  useEffect(() => {
    if (height == null) return
    const root = document.documentElement
    root.style.setProperty('--voice-dock', `${height}px`)
    return () => {
      root.style.removeProperty('--voice-dock')
    }
  }, [height])
  const h = height ?? VOICE_PANEL_SIZE.bar
  // the chat is hidden while the friends panel is open, so the panel then sits at the bottom
  const chatHeight = friends.open ? 0 : chat.open ? SHARED_AREA - PANEL_GAP - h : CHAT_INPUT_ROW
  return { '--panel-h': `${h}px`, '--chat-h': `${chatHeight}px` } as React.CSSProperties
}

export function VoiceChatPanel(): React.JSX.Element | null {
  const { voiceChat } = useSession()
  const state = voiceChat.state
  const [collapsed, setCollapsed] = useState(false)
  const [people, setPeople] = useState(false)
  const visible = state != null && state.status !== 'disconnected'
  const speakers = state?.participants.filter((p) => p.isSpeaker).length ?? 0
  const height = !visible
    ? null
    : state.kind !== 'community' || collapsed
      ? VOICE_PANEL_SIZE.bar
      : speakers <= VOICE_PANEL_SIZE.maxSpeakersPerLine && !people
        ? VOICE_PANEL_SIZE.expanded1
        : VOICE_PANEL_SIZE.expanded2
  const dock = useVoiceDock(height)
  // a new stream opens expanded on the speakers
  const key = state?.callId ?? state?.communityId ?? ''
  useEffect(() => {
    setCollapsed(false)
    setPeople(false)
  }, [key])
  if (!visible) return null
  return (
    <div className={styles.root} style={dock} role="dialog" aria-label={state.kind === 'community' ? 'Community stream' : 'Voice call'}>
      {state.kind === 'community' ? (
        <CommunityStream state={state} collapsed={collapsed} setCollapsed={setCollapsed} people={people} setPeople={setPeople} />
      ) : (
        <PrivateCall state={state} />
      )}
    </div>
  )
}

function peerName(address: string | null, fallback: string): { name: string; picture?: string; claimed: boolean; color: string } {
  const profile = useProfile(address ?? '')
  const name = profile?.name ?? fallback
  return { name, picture: profile?.picture, claimed: profile?.hasClaimedName ?? false, color: knownUserColor(address ?? '', name) }
}


/** The microphone button: a round 28px toggle; the clip plays from the engine. */
function MicButton({ enabled, onToggle, disabled = false }: { enabled: boolean; onToggle: () => void; disabled?: boolean }): React.JSX.Element {
  const label = enabled ? 'Mute microphone' : 'Unmute microphone'
  return (
    <Tooltip label={label} side="top">
      <ControlButton shape="circle" variant="solid" size="sm" className={enabled ? styles.micOn : styles.micOff} aria-pressed={enabled} aria-label={label} disabled={disabled} onClick={onToggle}>
        <MaskIcon src={enabled ? micOnIcon : micOffIcon} size={16} />
      </ControlButton>
    </Tooltip>
  )
}

/** The round red 28px button that ends, refuses or leaves. */
function HangUpButton({ label, onClick, disabled = false }: { label: string; onClick: () => void; disabled?: boolean }): React.JSX.Element {
  return (
    <Tooltip label={label} side="top">
      <ControlButton shape="circle" variant="solid" size="sm" className={styles.hangUp} aria-label={label} disabled={disabled} onClick={onClick}>
        <MaskIcon src={endCallIcon} size={16} />
      </ControlButton>
    </Tooltip>
  )
}

function PrivateCall({ state }: { state: VoiceChatState }): React.JSX.Element {
  const { voiceChat } = useSession()
  const act = voiceChat.act
  const remote = state.participants.find((p) => p.address.toLowerCase() === (state.peer ?? '').toLowerCase())
  const peer = peerName(state.peer, remote?.name || state.peer || 'Unknown')
  const { base, tag } = splitName(peer.name)
  const speaking = remote?.isSpeaking ?? false
  const ringing = state.status === 'receivedCall' || state.status === 'startedCall'
  const connected = state.status === 'inCall' && state.connection === 'connected'
  const failed = state.status === 'busy' || state.status === 'genericError'

  // The titlebar: the header line above the name (or the talking line in an ongoing call)
  let head = 'Voice call'
  let line: React.ReactNode = null
  if (state.status === 'receivedCall') head = 'Incoming Call from'
  else if (state.status === 'startingCall' && state.callId == null) head = 'Calling'
  else if (state.status === 'startedCall') head = 'Calling'
  else if (state.status === 'startingCall' || (state.status === 'inCall' && !connected)) head = state.connection === 'reconnecting' ? 'Reconnecting with' : 'Connecting with'
  else if (connected) {
    head = 'Ongoing Call'
    line = speaking ? `${base} talking` : '*Crickets*'
  } else if (state.status === 'endingCall' || state.status === 'rejectingCall') head = 'Ending call'
  else if (failed) {
    head = 'Something Went Wrong'
    line = state.error ?? (state.status === 'busy' ? `${base} is in another call.` : "A connection couldn't be established. Please try again.")
  }
  const nameLine = (
    <span className={styles.barName} style={{ color: peer.color }}>
      {base}
      {peer.claimed ? <VerifiedBadge size={12} /> : tag ? <span className={styles.barTag}>{tag}</span> : null}
    </span>
  )

  return (
    <div className={styles.bar} data-ringing={ringing}>
      <div className={styles.barLeft}>
        {failed ? (
          <MaskIcon src={callErrorIcon} size={24} className={styles.errorIcon} />
        ) : (
          <span className={styles.peer} data-speaking={speaking}>
            <Avatar src={peer.picture} name={base} color={peer.color} size={28} />
          </span>
        )}
        {/* the live region is the call's state, not the talking line under it, which would be
            re-announced on every speaking change; a failure is announced with its message */}
        <span className={styles.barText} role={failed ? 'alert' : undefined}>
          <span className={styles.barHead} role={failed ? undefined : 'status'}>{head}</span>
          {line == null ? nameLine : <span className={styles.barLine}>{line}</span>}
        </span>
      </div>
      <div className={styles.barRight}>
        {state.status === 'receivedCall' ? (
          <>
            <Tooltip label="Refuse" side="top">
              <ControlButton shape="circle" variant="solid" size="sm" className={styles.hangUp} aria-label="Refuse" onClick={() => act({ op: 'rejectCall' })}>
                <MaskIcon src={rejectCallIcon} size={14} />
              </ControlButton>
            </Tooltip>
            <Tooltip label="Accept" side="top">
              <ControlButton shape="circle" variant="solid" size="sm" className={styles.accept} aria-label="Accept" onClick={() => act({ op: 'acceptCall' })}>
                <MaskIcon src={callIcon} size={16} />
              </ControlButton>
            </Tooltip>
          </>
        ) : failed ? (
          <ControlButton variant="faint" size="sm" aria-label="Close" onClick={() => act({ op: 'dismissError' })}>
            ✕
          </ControlButton>
        ) : (
          <>
            {connected && <MicButton enabled={state.micEnabled} onToggle={() => act({ op: 'setMic', enabled: !state.micEnabled })} />}
            <HangUpButton label="Hang up" disabled={state.status === 'endingCall' || state.status === 'rejectingCall'} onClick={() => act({ op: 'endCall' })} />
          </>
        )}
      </div>
    </div>
  )
}

function isModerator(role: string): boolean {
  return role === 'moderator' || role === 'owner'
}

/** A speaker button's name: its own label hides the face's badges and the state line, so it says them. */
function speakerLabel(p: VoiceChatParticipant): string {
  const name = splitName(p.name || p.address).base
  return p.isSpeaking ? `${name}, speaking` : p.isMuted ? `${name}, muted` : name
}

function Face({ p, size }: { p: VoiceChatParticipant; size: number }): React.JSX.Element {
  const { base } = splitName(p.name || p.address)
  return (
    <span className={styles.speakerFace} data-speaking={p.isSpeaking}>
      <Avatar src={p.profilePictureUrl || undefined} name={base} color={knownUserColor(p.address, p.name)} size={size} />
      {p.isMuted && p.isSpeaker && (
        <span className={styles.badge} title="Muted" role="img" aria-label="Muted">
          <MaskIcon src={micOffIcon} size={10} />
        </span>
      )}
      {p.isRequestingToSpeak && !p.isSpeaker && (
        <span className={styles.badge} title="Wants to speak" role="img" aria-label="Wants to speak">
          <MaskIcon src={handIcon} size={10} />
        </span>
      )}
    </span>
  )
}

function ParticipantMenu({ p, me, moderator }: { p: VoiceChatParticipant; me: boolean; moderator: boolean }): React.JSX.Element | null {
  const { voiceChat } = useSession()
  const act = voiceChat.act
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const onDown = (e: PointerEvent): void => {
      if (ref.current != null && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('pointerdown', onDown, true)
    const off = registerCancelLayer(() => setOpen(false))
    return () => {
      document.removeEventListener('pointerdown', onDown, true)
      off()
    }
  }, [open])
  if (me) return null
  const viewProfile = (): void => {
    setOpen(false)
    const r = ref.current?.getBoundingClientRect()
    openProfileCard(p.address, r?.right ?? 0, r?.top ?? 0, { above: true })
  }
  const items: ContextMenuItem[] = [{ label: 'View profile', onClick: viewProfile }]
  if (moderator && !isModerator(p.role)) {
    items.push(
      p.isSpeaker
        ? { label: 'Demote to listener', onClick: () => { setOpen(false); act({ op: 'demoteSpeaker', address: p.address }) } }
        : { label: 'Promote to speaker', onClick: () => { setOpen(false); act({ op: 'promoteSpeaker', address: p.address }) } },
      { label: 'Remove from stream', danger: true, onClick: () => { setOpen(false); act({ op: 'kickPlayer', address: p.address }) } }
    )
  }
  return (
    <div ref={ref} className={styles.menuWrap}>
      <ControlButton variant="faint" size="sm" aria-label={`Options for ${splitName(p.name || p.address).base}`} aria-expanded={open} active={open} onClick={() => setOpen((o) => !o)}>
        <Kebab vertical size={16} r={2} />
      </ControlButton>
      {open && (
        <div className={styles.menu}>
          <ContextMenu items={items} />
        </div>
      )}
    </div>
  )
}

function CommunityStream({ state, collapsed, setCollapsed, people, setPeople }: {
  state: VoiceChatState
  collapsed: boolean
  setCollapsed: (c: boolean) => void
  people: boolean
  setPeople: (p: boolean) => void
}): React.JSX.Element {
  const { voiceChat, communities, profile } = useSession()
  const act = voiceChat.act
  const [query, setQuery] = useState('')
  const me = profile.data?.address.toLowerCase() ?? ''
  const community = communities.list.find((c) => c.id === state.communityId)
  const live = voiceChat.active.find((c) => c.communityId === state.communityId)
  const name = community?.name ?? live?.communityName ?? 'Community stream'
  const thumbnail = community?.thumbnail ?? live?.communityImage
  const moderator = isModerator(state.localRole)
  const connected = state.status === 'inCall' && state.connection === 'connected'

  const { speakers, hands, listeners, talking } = useMemo(() => {
    const speakers = state.participants.filter((p) => p.isSpeaker)
    const hands = state.participants.filter((p) => !p.isSpeaker && p.isRequestingToSpeak)
    const q = query.trim().toLowerCase()
    const listeners = state.participants.filter((p) => !p.isSpeaker && (q === '' || (p.name || p.address).toLowerCase().includes(q)))
    const talking = speakers.filter((p) => p.isSpeaking)
    return { speakers, hands, listeners, talking }
  }, [state.participants, query])

  const openProfile = (p: VoiceChatParticipant) => (e: React.MouseEvent): void => {
    openProfileCard(p.address, e.clientX, e.clientY)
  }
  const shown = (p: VoiceChatParticipant): string => (p.address.toLowerCase() === me ? 'You' : splitName(p.name || p.address).base)

  // The header: the people count, then who is talking (Playername / Several people / *Crickets*)
  const status = !voiceActive(state)
    ? null
    : state.status === 'startingCall' || !connected
      ? state.connection === 'reconnecting'
        ? 'Reconnecting'
        : 'Connecting'
      : `${state.participants.length} · ${talking.length === 0 ? '*Crickets*' : talking.length === 1 ? `${shown(talking[0])} talking` : 'Several people talking'}`

  // The controls: mic (speakers) or raise / lower hand, leave stage, hang up
  const controls = (
    <>
      {state.localSpeaker ? (
        <MicButton enabled={state.micEnabled} disabled={!connected} onToggle={() => act({ op: 'setMic', enabled: !state.micEnabled })} />
      ) : (
        <Tooltip label={state.localRequestingToSpeak ? 'Lower hand' : 'Raise hand'} side="top">
          <ControlButton shape="circle" variant="solid" size="sm" className={state.localRequestingToSpeak ? styles.handUp : ''} disabled={!connected} aria-pressed={state.localRequestingToSpeak} aria-label={state.localRequestingToSpeak ? 'Lower hand' : 'Raise hand'} onClick={() => act({ op: 'requestToSpeak', raise: !state.localRequestingToSpeak })}>
            <MaskIcon src={raiseHandIcon} size={16} />
          </ControlButton>
        </Tooltip>
      )}
      {state.localSpeaker && !moderator && (
        <Tooltip label="Leave stage" side="top">
          <ControlButton shape="circle" variant="solid" size="sm" disabled={!connected} aria-label="Leave stage" onClick={() => act({ op: 'demoteSpeaker', address: me })}>
            <MaskIcon src={leaveStageIcon} size={16} />
          </ControlButton>
        </Tooltip>
      )}
      <HangUpButton label="Leave" onClick={() => act({ op: 'leaveCall' })} />
    </>
  )

  return (
    <>
      <div className={styles.head}>
        <Avatar src={thumbnail} name={name} color={nameColor(state.communityId ?? name)} size={30} />
        <div className={styles.headTitle}>
          <span className={styles.title}>{name}</span>
          {status != null && <span className={styles.subtitle}>{status}</span>}
          {/* announces the connection only: the subtitle also names who talks, on every change */}
          <span className={styles.srOnly} role="status">
            {!voiceActive(state) ? '' : connected ? 'Connected' : state.connection === 'reconnecting' ? 'Reconnecting' : 'Connecting'}
          </span>
        </div>
        <div className={styles.headActions}>
          {collapsed && controls}
          {moderator && !collapsed && (
            <Tooltip label="End stream for everyone" side="bottom">
              <ControlButton variant="faint" size="sm" aria-label="End stream" onClick={() => act({ op: 'endCommunityCall' })}>
                <MaskIcon src={endStreamIcon} size={16} />
              </ControlButton>
            </Tooltip>
          )}
          <ControlButton variant="faint" size="sm" aria-label={collapsed ? 'Expand stream panel' : 'Collapse stream panel'} aria-expanded={!collapsed} onClick={() => setCollapsed(!collapsed)}>
            {collapsed ? '▾' : '▴'}
          </ControlButton>
        </div>
      </div>

      {collapsed ? null : people ? (
        <>
          {/* The People view: "People", back, the search, the sections */}
          <div className={styles.sectionHead}>
            <ControlButton variant="faint" size="sm" aria-label="Back to speakers" onClick={() => setPeople(false)}>
              ‹
            </ControlButton>
            People <span className={styles.sectionCount}>{state.participants.length}</span>
            <div className={styles.search}>
              <SearchField size="sm" value={query} onChange={setQuery} placeholder="Search listeners" />
            </div>
          </div>
          <div className={styles.body}>
            {moderator && hands.length > 0 && (
              <>
                <div className={styles.sectionHead}>
                  Raised hands <span className={styles.sectionCount}>{hands.length}</span>
                </div>
                {hands.map((p) => (
                  <div key={p.address} className={styles.row}>
                    <Face p={p} size={30} />
                    <span className={styles.rowName}>{splitName(p.name || p.address).base}</span>
                    <div className={styles.rowActions}>
                      {/* The 20px Approve / Deny icons */}
                      <Tooltip label="Deny" side="top">
                        <ControlButton variant="ghost" shape="circle" size="sm" aria-label={`Deny ${splitName(p.name || p.address).base}`} onClick={() => act({ op: 'rejectSpeakRequest', address: p.address })}>
                          <img src={rejectRequestIcon} alt="" width={20} height={20} />
                        </ControlButton>
                      </Tooltip>
                      <Tooltip label="Approve" side="top">
                        <ControlButton variant="ghost" shape="circle" size="sm" aria-label={`Approve ${splitName(p.name || p.address).base}`} onClick={() => act({ op: 'promoteSpeaker', address: p.address })}>
                          <img src={acceptRequestIcon} alt="" width={20} height={20} />
                        </ControlButton>
                      </Tooltip>
                    </div>
                  </div>
                ))}
              </>
            )}
            {/* speakers carry the listeners' menu, so a moderator can demote or remove them */}
            {speakers.length > 0 && (
              <>
                <div className={styles.sectionHead}>
                  Speakers <span className={styles.sectionCount}>{speakers.length}</span>
                </div>
                {speakers.map((p) => (
                  <div key={p.address} className={styles.row}>
                    <Face p={p} size={30} />
                    <span className={styles.rowName}>{shown(p)}</span>
                    {isModerator(p.role) && <span className={styles.rowRole}>{p.role}</span>}
                    <ParticipantMenu p={p} me={p.address.toLowerCase() === me} moderator={moderator} />
                  </div>
                ))}
              </>
            )}
            <div className={styles.sectionHead}>
              Listeners <span className={styles.sectionCount}>{state.participants.filter((p) => !p.isSpeaker).length}</span>
            </div>
            {listeners.length === 0 ? (
              <div className={styles.empty}>{query ? 'Nobody matches.' : 'Nobody is listening yet.'}</div>
            ) : (
              listeners.map((p) => (
                <div key={p.address} className={styles.row}>
                  <Face p={p} size={30} />
                  <span className={styles.rowName}>{shown(p)}</span>
                  {isModerator(p.role) && <span className={styles.rowRole}>{p.role}</span>}
                  <ParticipantMenu p={p} me={p.address.toLowerCase() === me} moderator={moderator} />
                </div>
              ))
            )}
          </div>
        </>
      ) : (
        <div className={styles.body}>
          <div className={styles.sectionHead}>
            Speakers <span className={styles.sectionCount}>{speakers.length}</span>
          </div>
          {speakers.length === 0 ? (
            <div className={styles.empty}>No speakers yet.</div>
          ) : (
            <div className={styles.speakers} role="list" aria-label="Speakers">
              {speakers.slice(0, MAX_VISIBLE_SPEAKERS).map((p) => (
                <div key={p.address} role="listitem" className={styles.speakerItem}>
                  <button type="button" className={styles.speaker} aria-label={speakerLabel(p)} onClick={openProfile(p)}>
                    <Face p={p} size={40} />
                    <span className={styles.speakerName}>{shown(p)}</span>
                    <span className={styles.speakerState}>
                      {p.isSpeaking ? (
                        <>
                          <img src={speakingFeedbackIcon} alt="" width={10} height={11} /> Speaking
                        </>
                      ) : p.isMuted ? (
                        'Muted'
                      ) : null}
                    </span>
                  </button>
                </div>
              ))}
              {speakers.length > MAX_VISIBLE_SPEAKERS && (
                <div role="listitem" className={styles.more}>+{speakers.length - MAX_VISIBLE_SPEAKERS}</div>
              )}
            </div>
          )}
        </div>
      )}
      {!collapsed && (
        <div className={styles.foot}>
          <Tooltip label={moderator && hands.length > 0 ? `${hands.length} raised ${hands.length === 1 ? 'hand' : 'hands'}` : 'People'} side="right">
            <ControlButton shape="circle" variant="solid" size="sm" className={styles.people} active={people} aria-label="People" onClick={() => setPeople(!people)}>
              <MaskIcon src={peopleIcon} size={16} />
              {moderator && hands.length > 0 && (
                <span className={styles.handsBadge}>
                  <MaskIcon src={handNotificationIcon} size={9} />
                  {hands.length}
                </span>
              )}
            </ControlButton>
          </Tooltip>
          <span className={styles.footSpacer} />
          {controls}
        </div>
      )}
    </>
  )
}
