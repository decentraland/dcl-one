// The route to the destination: a dotted line from the destination back to the player, and the
// destination's pin.

import { useEffect, useRef, type CSSProperties } from 'react'
import { playerMarkerPosition } from '@explorer-core/features/map/mapCoords'
import type { Destination } from '@explorer-core/features/map/navigation'
import type { PlayerPose } from '../session/useEngineSession'
import { MARKER_ART } from './mapArt'
import styles from './MapCanvas.module.css'

export function RouteLayer({ destination, pose, active }: { destination: Destination | null; pose: { current: PlayerPose }; active: boolean }): React.JSX.Element | null {
  const line = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!active || destination == null) return
    let frame = 0
    const tick = (): void => {
      const el = line.current
      if (el != null) {
        const from = playerMarkerPosition(pose.current.x, pose.current.z)
        const dx = from.x - destination.position.x
        const dy = from.y - destination.position.y
        el.style.setProperty('--length', String(Math.hypot(dx, dy)))
        // screen y runs down, so the map angle is mirrored
        el.style.setProperty('--angle', `${-Math.atan2(dy, dx)}rad`)
      }
      frame = requestAnimationFrame(tick)
    }
    tick()
    return () => cancelAnimationFrame(frame)
  }, [destination, pose, active])

  if (destination == null) return null
  const at = { '--x': destination.position.x, '--y': destination.position.y } as CSSProperties
  return (
    <>
      <div ref={line} className={`${styles.marker} ${styles.route}`} style={at} />
      <div className={`${styles.marker} ${styles.destinationPin}`} style={at} role="img" aria-label="Destination">
        <img className={styles.destinationEllipse} src={MARKER_ART.destinationEllipse} alt="" />
        <img className={styles.destinationArt} src={MARKER_ART.destination} alt="" />
      </div>
    </>
  )
}
