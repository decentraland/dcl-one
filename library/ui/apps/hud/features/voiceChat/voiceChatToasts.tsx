// Toasts for voice chat: a community stream ending (the social service's broadcast; its start comes
// as a `community_voice_chat_started` notification, toasted with the other notifications), and
// the errors the engine reports outside the call panel's own Busy / Error views (a refused
// moderation action, a guest trying to call), and how a call room went away.
import { useEffect, useRef } from 'react'
import { showToast } from '../../design'
import type { VoiceChatState } from '@explorer-core/engine/protocol'
import { useSession } from '../session/SessionContext'
import { onVoiceChatEvent } from '../session/useEngineSession'

/** Mounted at the top level, beside the notification toasts: loading screens, teleports, the
 *  engine menu and full-screen pages must not unmount it, or the broadcasts in between are lost
 *  and the last error is shown again on remount. */
export function VoiceChatToasts(): null {
  useVoiceChatToasts(useSession().voiceChat.state)
  return null
}

/** Subscribes for the component's lifetime; mount once (`VoiceChatToasts` does). */
export function useVoiceChatToasts(state: VoiceChatState | null): void {
  useEffect(
    () =>
      onVoiceChatEvent((e) => {
        // a public stream is also broadcast to non-members, as the sidebar badge knows
        if (!e.isMember || e.status !== 'ended') return
        showToast(
          <>
            <b>{e.communityName || 'A community'}</b>&rsquo;s voice stream ended.
          </>
        )
      }),
    []
  )
  // An error while the panel shows a Busy / Error view is that view's; any other is a toast. Each
  // error has its own `errorSeq`, so the same refusal twice is two toasts, and an error already
  // shown is not shown again.
  const lastSeq = useRef<number | null>(null)
  useEffect(() => {
    if (state == null) return
    if (lastSeq.current == null) {
      // first snapshot after mount: whatever error it carries happened before
      lastSeq.current = state.errorSeq
      return
    }
    if (state.errorSeq === lastSeq.current) return
    lastSeq.current = state.errorSeq
    if (state.error == null) return
    if (state.status === 'busy' || state.status === 'genericError') return
    showToast(state.error, { tone: state.errorIsNotice ? 'default' : 'error' })
  }, [state])
}
