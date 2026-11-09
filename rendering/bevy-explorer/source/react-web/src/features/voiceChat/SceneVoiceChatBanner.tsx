// "A stream is happening here": the live community stream whose places include the parcel or the
// world the player stands in (unity-explorer's SceneVoiceChatActiveCallView), with a Join button.
import { Avatar, Button, MaskIcon } from '../../design'
import joinStreamIcon from '../../assets/voice/join-stream.png'
import streamIcon from '../../assets/voice/stream-small.png'
import { nameColor } from '../../lib/identity'
import { useSession } from '../session/SessionContext'
import { inCommunityStream, streamAct, voiceActive } from './callGate'
import { VOICE_PANEL_SIZE, useVoiceDock } from './VoiceChatPanel'
import styles from './VoiceChatPanel.module.css'

export function SceneVoiceChatBanner(): React.JSX.Element | null {
  const { voiceChat } = useSession()
  const chat = voiceChat.scene
  const state = voiceChat.state
  // the panel shows it once joined; another call (or its error) keeps the panel's dock
  const shown = chat != null && !voiceChat.guest && !inCommunityStream(state, chat.communityId) && !voiceActive(state) && (state == null || state.status === 'disconnected')
  const dock = useVoiceDock(shown ? VOICE_PANEL_SIZE.scene : null)
  if (chat == null || !shown) return null
  return (
    <div className={`${styles.root} ${styles.banner}`} style={dock} role="status" aria-label="Stream in this scene">
      <Avatar src={chat.communityImage} name={chat.communityName} color={nameColor(chat.communityId)} size={32} />
      <span className={styles.bannerText}>
        <b className={styles.bannerName}>{chat.communityName}</b>
        <span className={styles.bannerSub}>
          <MaskIcon src={streamIcon} size={12} /> Live Voice Stream · {chat.participantCount}
        </span>
      </span>
      <Button size="sm" onClick={() => streamAct(voiceChat, { op: 'joinCommunityCall', communityId: chat.communityId })}>
        <MaskIcon src={joinStreamIcon} size={14} /> Join stream
      </Button>
    </div>
  )
}
