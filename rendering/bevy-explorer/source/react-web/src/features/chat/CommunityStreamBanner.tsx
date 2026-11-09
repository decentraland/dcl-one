import { MaskIcon, showToast } from '../../design'
import { useOptionalSession } from '../session/SessionContext'
import { inCommunityStream, streamGate } from '../voiceChat/callGate'
import playersIcon from '../../assets/chat/players.png'
import joinStreamIcon from '../../assets/voice/join-stream.png'
import streamIcon from '../../assets/voice/stream-large.png'
import styles from './Chat.module.css'

export function CommunityStreamBanner({ communityId }: { communityId: string }): React.JSX.Element | null {
  const session = useOptionalSession()
  // the chat also renders without a session
  if (session == null) return null
  const { voiceChat } = session
  const live = voiceChat.active.find((c) => c.communityId === communityId)
  if (live == null || inCommunityStream(voiceChat.state, communityId)) return null
  const join = (): void => {
    const gate = streamGate(voiceChat.state, session.profile.data?.isGuest === true)
    if (!gate.allowed) showToast(gate.reason, { tone: 'error' })
    else voiceChat.act({ op: 'joinCommunityCall', communityId })
  }
  return (
    <button type="button" className={styles.streamBanner} aria-label={`Join the live voice stream, ${live.participantCount} listening`} onClick={join}>
      <span className={styles.streamLive}>
        <MaskIcon src={streamIcon} size={16} />
        Live Voice Stream
        <span className={styles.streamCount}>
          <MaskIcon src={playersIcon} size={13} />
          {live.participantCount}
        </span>
      </span>
      <span className={styles.streamJoin}>
        <MaskIcon src={joinStreamIcon} size={15} />
        JOIN STREAM
      </span>
    </button>
  )
}
