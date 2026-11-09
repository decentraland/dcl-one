// Toasts for voice chat: a community stream starting or ending (the social service's broadcast), and
// the errors the engine reports outside the call panel's own Busy / Error views (a refused
// moderation action, a guest trying to call).
import { useEffect, useRef } from 'react'
import { showToast } from '../../design'
import type { VoiceChatState } from '../../engine/protocol'
import { useSession } from '../session/SessionContext'
import { onVoiceChatEvent } from '../session/useEngineSession'

/** Mounted outside the chrome: a full-screen page must not unmount it. */
export function VoiceChatToasts(): null {
  useVoiceChatToasts(useSession().voiceChat.state)
  return null
}

/** Subscribes for the component's lifetime; mount once (`VoiceChatToasts` does). */
export function useVoiceChatToasts(state: VoiceChatState | null): void {
  useEffect(
    () =>
      onVoiceChatEvent((e) => {
        if (e.status === 'started') {
          showToast(
            <>
              <b>{e.communityName || 'A community'}</b> started a voice stream.
            </>
          )
        } else {
          showToast(
            <>
              <b>{e.communityName || 'A community'}</b>&rsquo;s voice stream ended.
            </>
          )
        }
      }),
    []
  )
  // An error while the panel shows a Busy / Error view is that view's; any other is a toast.
  const lastError = useRef<string | null>(null)
  useEffect(() => {
    const error = state?.error ?? null
    if (error === lastError.current) return
    lastError.current = error
    if (error == null || state == null) return
    if (state.status === 'busy' || state.status === 'genericError') return
    showToast(error, { tone: 'error' })
  }, [state])
}
