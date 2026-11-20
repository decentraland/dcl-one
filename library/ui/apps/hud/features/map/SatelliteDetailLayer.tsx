// The deep-zoom satellite imagery, drawn over the base chunks in the same map units. The camera
// says what is in view; the store (explorer-core's satelliteTiles) loads and keeps the tiles.

import { useEffect, useMemo, useSyncExternalStore, type CSSProperties } from 'react'
import { SatelliteDetail } from '@explorer-core/features/map/satelliteTiles'
import { PARCEL_UNITS } from '@explorer-core/features/map/mapCoords'
import type { MapCamera } from './useMapCamera'
import styles from './MapCanvas.module.css'

const EMPTY: readonly never[] = []
const noTiles = (): readonly never[] => EMPTY
const noSubscribe = (): (() => void) => () => {}

export function SatelliteDetailLayer({
  camera,
  enabled,
  fetchTile
}: {
  camera: MapCamera
  /** Genesis City's satellite map is showing. */
  enabled: boolean
  /** The engine's tile decoder; without one the map keeps its base imagery. */
  fetchTile: ((path: string) => Promise<string | null>) | undefined
}): React.JSX.Element | null {
  const store = useMemo(() => (fetchTile == null ? null : new SatelliteDetail({ fetchTile })), [fetchTile])
  useEffect(() => () => store?.dispose(), [store])

  const { detail } = camera
  const view = useSyncExternalStore(detail.subscribe, detail.getSnapshot)
  useEffect(() => {
    store?.setView(enabled ? view : null)
  }, [store, enabled, view])

  const tiles = useSyncExternalStore(store?.subscribe ?? noSubscribe, store?.getSnapshot ?? noTiles)
  if (!enabled || tiles.length === 0) return null
  return (
    <div className={styles.world}>
      {tiles.map((t) => (
        <img
          key={t.key}
          className={styles.chunk}
          style={{ '--cx': ((t.rect.xMin + t.rect.xMax) / 2) * PARCEL_UNITS, '--cy': ((t.rect.yMin + t.rect.yMax) / 2) * PARCEL_UNITS, '--size': (t.rect.xMax - t.rect.xMin) * PARCEL_UNITS } as CSSProperties}
          src={t.url}
          alt=""
          draggable={false}
        />
      ))}
    </div>
  )
}
