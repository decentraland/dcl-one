// The map itself: the atlas, the parcel under the pointer, and every marker layer. Markers and
// atlas chunks carry their map position as custom properties and place themselves from the
// camera's, which useMapCamera writes on the viewport.

import { memo, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { GRID, PARCELS_CHUNK_PARCELS, PARCELS_PER_TILE, parcelsChunks, satelliteChunkCenter, tileUrl } from '@explorer-core/features/map/atlas'
import { clusterMarkers, type Cluster } from '@explorer-core/features/map/clusters'
import { PARCEL_UNITS, parcelCenter, parcelKey, playerMarkerPosition, type Vec2 } from '@explorer-core/features/map/mapCoords'
import type { MapLayer } from '@explorer-core/features/map/mapLayersStore'
import type { EventMarker, PlaceMarker } from '@explorer-core/features/map/mapMarkers'
import type { CategoryKey } from '@explorer-core/features/map/navmap'
import type { PlayerPose } from '../session/useEngineSession'
import { CAT_PINS, MARKER_ART } from './mapArt'
import { OceanLayer } from './OceanLayer'
import type { MapCamera } from './useMapCamera'
import styles from './MapCanvas.module.css'

const at = (pos: Vec2): CSSProperties => ({ '--x': pos.x, '--y': pos.y }) as CSSProperties

/** Pointer travel, in client px, before a press becomes a drag. */
const DRAG_THRESHOLD = 10

type Bounds = MapCamera['visible']

const SATELLITE_CHUNKS = Array.from({ length: GRID * GRID }, (_, i) => {
  const col = Math.floor(i / GRID)
  const row = i % GRID
  const center = satelliteChunkCenter(col, row)
  return { key: `${col},${row}`, url: tileUrl(col, row), x: center.x * PARCEL_UNITS, y: center.y * PARCEL_UNITS, size: PARCELS_PER_TILE * PARCEL_UNITS }
})

// The satellite imagery carries a border beyond the city; these cover it with the shore's sand,
// leaving the city and its two strips of extra parcels. Centre and size, map units.
const EDGE_PATCHES = [
  { x: -3320, y: -20, w: 600, h: 7200 }, // west of the city
  { x: 280, y: -3320, w: 6600, h: 600 }, // south
  { x: 3290, y: -930, w: 580, h: 4180 }, // east, south of the east strip
  { x: 3420, y: 2080, w: 320, h: 1840 }, // east of the east strip
  { x: -900, y: 3020, w: 4240, h: 40 }, // north, west of the north strip
  { x: 3410, y: 3020, w: 340, h: 40 }, // north, east of the north strip
  { x: 280, y: 3310, w: 6600, h: 540 } // north of the imagery
]

interface Chunk {
  key: string
  url: string
  /** Centre and side, map units. */
  x: number
  y: number
  size: number
}

const inView = (c: Chunk, b: Bounds): boolean => c.x + c.size / 2 > b.xMin && c.x - c.size / 2 < b.xMax && c.y + c.size / 2 > b.yMin && c.y - c.size / 2 < b.yMax

function AtlasChunk({ chunk, loadingTile }: { chunk: Chunk; loadingTile: boolean }): React.JSX.Element {
  const [loaded, setLoaded] = useState(false)
  return (
    <div className={styles.chunk} style={{ '--cx': chunk.x, '--cy': chunk.y, '--size': chunk.size } as CSSProperties}>
      {loadingTile && <div className={`${styles.chunkLoading} ${loaded ? styles.chunkLoadingDone : ''}`.trim()} />}
      <img
        className={`${styles.chunkImage} ${loaded ? styles.chunkImageLoaded : ''}`.trim()}
        src={chunk.url}
        alt=""
        draggable={false}
        onLoad={() => setLoaded(true)}
      />
    </div>
  )
}

const Atlas = memo(function Atlas({ chunks, visible, loadingTile }: { chunks: readonly Chunk[]; visible: Bounds; loadingTile: boolean }): React.JSX.Element {
  // a chunk stays mounted once it has been in view, so panning back does not fade it in again
  const seen = useRef(new Set<string>())
  for (const chunk of chunks) if (inView(chunk, visible)) seen.current.add(chunk.key)
  return (
    <div className={styles.world}>
      {chunks
        .filter((c) => seen.current.has(c.key))
        .map((c) => (
          <AtlasChunk key={c.key} chunk={c} loadingTile={loadingTile} />
        ))}
    </div>
  )
})

function useParcelsChunks(enabled: boolean): readonly Chunk[] {
  // built on the first switch to the parcels map and kept from then on
  const [built, setBuilt] = useState(false)
  useEffect(() => {
    if (enabled) setBuilt(true)
  }, [enabled])
  return useMemo(
    () =>
      built
        ? parcelsChunks().map((c) => {
            const center = parcelCenter(c)
            return { key: `${c.x},${c.y}`, url: c.url, x: center.x, y: center.y, size: PARCELS_CHUNK_PARCELS * PARCEL_UNITS }
          })
        : [],
    [built]
  )
}

type PinKind = 'category' | 'poi' | 'search'

interface PinProps {
  kind: PinKind
  position: Vec2
  title: string
  icon: string
  selected: boolean
  /** The results list is pointing at it. */
  hot: boolean
  layer: string
  onClick: () => void
}

const Pin = memo(function Pin({ kind, position, title, icon, selected, hot, layer, onClick }: PinProps): React.JSX.Element {
  return (
    <button
      type="button"
      className={`${styles.marker} ${styles[kind]} ${selected ? styles.selected : ''} ${hot ? styles.hot : ''}`.trim()}
      style={at(position)}
      data-marker={layer}
      aria-label={title}
      aria-pressed={selected}
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
    >
      <span className={styles.scaler}>
        <img className={styles.pinIcon} src={selected ? MARKER_ART.focused : icon} alt="" draggable={false} />
      </span>
      <span className={styles.title}>{title}</span>
    </button>
  )
})

const ClusterPin = memo(function ClusterPin({ cluster, icon, search, label, onClick }: { cluster: Cluster; icon: string; search: boolean; label: string; onClick: () => void }): React.JSX.Element {
  return (
    <button
      type="button"
      className={`${styles.marker} ${styles.cluster} ${search ? styles.clusterSearch : ''}`.trim()}
      style={at(cluster.position)}
      data-marker="cluster"
      aria-label={`${cluster.count} ${label}`}
      onClick={(e) => {
        e.stopPropagation()
        onClick()
      }}
    >
      <span className={styles.scaler}>
        <img className={styles.clusterIcon} src={icon} alt="" draggable={false} />
        <span className={styles.clusterCount}>{cluster.count}</span>
      </span>
    </button>
  )
})

interface PinLayerProps<T extends { id: string; key: Vec2; position: Vec2; title: string }> {
  markers: readonly T[]
  level: number
  kind: PinKind
  layer: string
  icon: string
  label: string
  selected: string | null
  hot?: string | null
  hotSelected?: string | null
  onPick: (marker: T) => void
  onCluster: () => void
}

function PinLayer<T extends { id: string; key: Vec2; position: Vec2; title: string }>({
  markers,
  level,
  kind,
  layer,
  icon,
  label,
  selected,
  hot = null,
  hotSelected = null,
  onPick,
  onCluster
}: PinLayerProps<T>): React.JSX.Element {
  const { visible, clusters } = useMemo(() => clusterMarkers(markers, level), [markers, level])
  return (
    <div className={`${styles.layer} ${styles[`layer_${layer}`] ?? ''}`.trim()}>
      {clusters.map((c) => (
        <ClusterPin key={`c${c.id}`} cluster={c} icon={icon} search={kind === 'search'} label={label} onClick={onCluster} />
      ))}
      {visible.map((m) => (
        <Pin
          key={m.id}
          kind={kind}
          layer={layer}
          position={m.position}
          title={m.title}
          icon={icon}
          selected={selected === `${layer}:${m.id}` || hotSelected === parcelKey(m.key)}
          hot={hot === parcelKey(m.key)}
          onClick={() => onPick(m)}
        />
      ))}
    </div>
  )
}

function PlayerMarker({ pose, active }: { pose: { current: PlayerPose }; active: boolean }): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!active) return
    let frame = 0
    const tick = (): void => {
      const el = ref.current
      if (el != null) {
        const p = pose.current
        const pos = playerMarkerPosition(p.x, p.z)
        el.style.setProperty('--x', String(pos.x))
        el.style.setProperty('--y', String(pos.y))
        el.style.setProperty('--yaw', `${p.yaw}deg`)
      }
      frame = requestAnimationFrame(tick)
    }
    tick()
    return () => cancelAnimationFrame(frame)
  }, [pose, active])
  return (
    <div ref={ref} className={`${styles.marker} ${styles.player}`} role="img" aria-label="You are here">
      <span className={styles.playerRing} />
      <img className={styles.playerCompass} src={MARKER_ART.compass} alt="" draggable={false} />
    </div>
  )
}

export interface MapCanvasProps {
  camera: MapCamera
  open: boolean
  layers: ReadonlySet<MapLayer>
  /** The player is in Genesis City, so they have a place on this map. */
  genesis: boolean
  pose: { current: PlayerPose }
  liveEvents: readonly EventMarker[]
  pois: readonly PlaceMarker[]
  searchMarkers: readonly PlaceMarker[]
  categoryMarkers: readonly PlaceMarker[]
  markerCategory: CategoryKey | null
  categoryLayer: boolean
  /** Other players, as map positions. */
  people: readonly Vec2[]
  /** The home parcel, when home is in Genesis City. */
  home: Vec2 | null
  /** `layer:id` of the marker last clicked. */
  selected: string | null
  resultHover: string | null
  resultSelected: string | null
  /** Extra layers drawn among the markers (scene pins, the route). */
  children?: React.ReactNode
  onParcel: (parcel: Vec2) => void
  onEvent: (marker: EventMarker) => void
  onPlace: (layer: 'pois' | 'search' | 'category', marker: PlaceMarker) => void
  onHome: () => void
}

export function MapCanvas(props: MapCanvasProps): React.JSX.Element {
  const { camera, open, layers, genesis, pose, selected } = props
  const [highlight, setHighlight] = useState<Vec2 | null>(null)
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null)
  const dragged = useRef(false)

  const parcelsOn = layers.has('parcels')
  const parcels = useParcelsChunks(parcelsOn)

  const overMarker = (target: EventTarget | null): boolean => target instanceof Element && target.closest('[data-marker]') != null

  const onPointerDown = (e: React.PointerEvent): void => {
    if (e.button !== 0) return
    press.current = { x: e.clientX, y: e.clientY, dragging: false }
    dragged.current = false
  }
  const onPointerMove = (e: React.PointerEvent): void => {
    const p = press.current
    if (p != null) {
      if (!p.dragging && Math.hypot(e.clientX - p.x, e.clientY - p.y) > DRAG_THRESHOLD) {
        p.dragging = true
        dragged.current = true
        camera.beginDrag(p.x, p.y)
        e.currentTarget.setPointerCapture?.(e.pointerId)
      }
      if (p.dragging) {
        camera.drag(e.clientX, e.clientY)
        return
      }
    }
    const parcel = overMarker(e.target) ? null : camera.parcelAt(e.clientX, e.clientY)
    setHighlight((prev) => (prev?.x === parcel?.x && prev?.y === parcel?.y ? prev : parcel))
  }
  const endPress = (e: React.PointerEvent): void => {
    if (press.current?.dragging) {
      camera.endDrag()
      e.currentTarget.releasePointerCapture?.(e.pointerId)
    }
    press.current = null
  }
  // a click that ended a drag is not a click on the map; markers stop theirs from reaching here
  const onClick = (e: React.MouseEvent): void => {
    if (dragged.current) {
      dragged.current = false
      return
    }
    const parcel = camera.parcelAt(e.clientX, e.clientY)
    if (parcel != null) props.onParcel(parcel)
  }
  const onMarkerClickCapture = (e: React.MouseEvent): void => {
    if (dragged.current && overMarker(e.target)) {
      dragged.current = false
      e.stopPropagation()
    }
  }
  const onWheel = (e: React.WheelEvent): void => {
    if (e.deltaY !== 0) camera.zoom(e.deltaY < 0)
  }
  const zoomIn = (): void => camera.zoom(true)

  return (
    <div
      ref={camera.viewportRef}
      data-testid="map-view"
      className={styles.viewport}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endPress}
      onPointerCancel={endPress}
      onPointerLeave={() => setHighlight(null)}
      onClickCapture={onMarkerClickCapture}
      onClick={onClick}
      onWheel={onWheel}
    >
      {layers.has('satellite') && <OceanLayer view={camera.view} />}
      {parcelsOn && <Atlas chunks={parcels} visible={camera.visible} loadingTile />}
      {layers.has('satellite') && (
        <>
          <Atlas chunks={SATELLITE_CHUNKS} visible={camera.visible} loadingTile={false} />
          <div className={styles.world}>
            {EDGE_PATCHES.map((patch, i) => (
              <div key={i} className={styles.patch} style={{ '--cx': patch.x, '--cy': patch.y, '--w': patch.w, '--h': patch.h } as CSSProperties} />
            ))}
          </div>
        </>
      )}

      {layers.has('people') && (
        <div className={`${styles.layer} ${styles.layer_people}`}>
          {props.people.map((p, i) => (
            <span key={i} className={`${styles.marker} ${styles.user}`} style={at(p)} />
          ))}
        </div>
      )}
      <PinLayer
        markers={props.searchMarkers}
        level={camera.level}
        kind="search"
        layer="search"
        icon={MARKER_ART.generic}
        label="search results"
        selected={selected}
        hot={props.resultHover}
        hotSelected={props.resultSelected}
        onPick={(m) => props.onPlace('search', m)}
        onCluster={zoomIn}
      />
      {props.categoryLayer && (
        <PinLayer
          markers={props.categoryMarkers}
          level={camera.level}
          kind="category"
          layer="category"
          icon={(props.markerCategory != null ? CAT_PINS[props.markerCategory] : undefined) ?? MARKER_ART.generic}
          label="places"
          selected={selected}
          hot={props.resultHover}
          hotSelected={props.resultSelected}
          onPick={(m) => props.onPlace('category', m)}
          onCluster={zoomIn}
        />
      )}
      {genesis && <PlayerMarker pose={pose} active={open} />}
      {layers.has('pois') && (
        <PinLayer
          markers={props.pois}
          level={camera.level}
          kind="poi"
          layer="pois"
          icon={MARKER_ART.poi}
          label="points of interest"
          selected={selected}
          onPick={(m) => props.onPlace('pois', m)}
          onCluster={zoomIn}
        />
      )}
      {props.children}
      {props.home != null && (
        <button
          type="button"
          className={`${styles.marker} ${styles.home}`}
          style={at(parcelCenter(props.home))}
          data-marker="home"
          aria-label="Home"
          onClick={(e) => {
            e.stopPropagation()
            props.onHome()
          }}
        >
          <span className={styles.scaler}>
            <img className={styles.pinIcon} src={MARKER_ART.home} alt="" draggable={false} />
          </span>
        </button>
      )}
      {layers.has('liveEvents') && (
        <PinLayer
          markers={props.liveEvents}
          level={camera.level}
          kind="category"
          layer="liveEvents"
          icon={MARKER_ART.live}
          label="live events"
          selected={selected}
          onPick={props.onEvent}
          onCluster={zoomIn}
        />
      )}
      {highlight != null && (
        <div className={`${styles.marker} ${styles.highlight}`} style={at(parcelCenter(highlight))}>
          <img className={styles.highlightSprite} src={MARKER_ART.parcelSelector} alt="" draggable={false} />
          <span className={styles.highlightCoords}>
            {highlight.x}, {highlight.y}
          </span>
        </div>
      )}
    </div>
  )
}
