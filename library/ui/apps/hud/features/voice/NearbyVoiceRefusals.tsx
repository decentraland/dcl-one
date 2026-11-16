import { useEffect, useRef } from 'react'
import { showToast } from '../../design'
import { useSession } from '../session/SessionContext'

export const NEARBY_VOICE_BLOCKED = 'Nearby voice chat unavailable in this scene.'

/** Mounted at the top level, so a refusal is toasted while the sidebar is hidden or unmounted. */
export function NearbyVoiceRefusals(): null {
  useNearbyVoiceRefusals(useSession().mic.refusals)
  return null
}

export function useNearbyVoiceRefusals(refusals: number): void {
  // the count seen on mount was refused before
  const last = useRef(refusals)
  useEffect(() => {
    if (refusals > last.current) showToast(NEARBY_VOICE_BLOCKED)
    last.current = refusals
  }, [refusals])
}
