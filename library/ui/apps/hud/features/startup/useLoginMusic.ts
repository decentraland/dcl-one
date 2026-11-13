// The sign-in music: it plays from the first screen through the lobby and the loading
// screen, and stops once the player is in the world. The sign-in screens' footer mutes it, and the
// choice is kept. A browser plays sound only after the player has interacted with the page, so it
// starts on the first click or key press.
import { useCallback, useEffect, useState } from 'react'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import music from '../../assets/startup/login-music.mp3'

const VOLUME = 0.2

let audio: HTMLAudioElement | null = null
function player(): HTMLAudioElement {
  if (audio == null) {
    audio = new Audio(music)
    audio.loop = true
    audio.volume = VOLUME
  }
  return audio
}

function play(): void {
  try {
    void player()
      .play()
      ?.catch(() => undefined)
  } catch {
    // no audio here (a test page): the screens work without it
  }
}

export interface LoginMusic {
  muted: boolean
  toggleMute: () => void
}

export function useLoginMusic(active: boolean): LoginMusic {
  const [muted, setMuted] = useState(() => getPref(PREF.loginMusicMuted) === '1')
  const toggleMute = useCallback(() => {
    setMuted((was) => {
      setPref(PREF.loginMusicMuted, was ? '0' : '1')
      return !was
    })
  }, [])

  useEffect(() => {
    if (!active || muted) {
      audio?.pause()
      return
    }
    play()
    // the browser refused before any interaction: try again on the first one
    const start = (): void => {
      if (audio?.paused !== false) play()
    }
    window.addEventListener('pointerdown', start, true)
    window.addEventListener('keydown', start, true)
    return () => {
      window.removeEventListener('pointerdown', start, true)
      window.removeEventListener('keydown', start, true)
    }
  }, [active, muted])

  return { muted, toggleMute }
}
