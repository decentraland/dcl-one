// The Layers panel: which pins and details the map shows, and its map type.

import { useSyncExternalStore } from 'react'
import { mapLayers, type MapLayer } from '@explorer-core/features/map/mapLayersStore'
import { Toggle } from '../../design'
import { MAP_ICONS, MARKER_ART } from './mapArt'
import styles from './MapChrome.module.css'

function Row({ layer, label, icon, layers }: { layer: MapLayer; label: string; icon?: string; layers: ReadonlySet<MapLayer> }): React.JSX.Element {
  return (
    <div className={styles.layerRow}>
      {icon != null && <img className={styles.layerIcon} src={icon} alt="" />}
      <span className={styles.layerLabel}>{label}</span>
      <Toggle size="sm" surface="light" aria-label={label} checked={layers.has(layer)} onChange={(on) => mapLayers.set(layer, on)} />
    </div>
  )
}

function MapType({ type, label, icon, layers }: { type: 'satellite' | 'parcels'; label: string; icon: string; layers: ReadonlySet<MapLayer> }): React.JSX.Element {
  const on = layers.has(type)
  return (
    <button type="button" role="radio" aria-checked={on} className={styles.mapType} onClick={() => mapLayers.setMapType(type)}>
      <span className={`${styles.mapTypeIcon} ${on ? styles.mapTypeOn : ''}`.trim()}>
        <img src={icon} alt="" />
      </span>
      <span className={styles.mapTypeLabel}>{label}</span>
    </button>
  )
}

export function LayersPanel(): React.JSX.Element {
  const layers = useSyncExternalStore(mapLayers.subscribe, mapLayers.getSnapshot)
  return (
    <section className={styles.layers} aria-label="Layers">
      <h2 className={styles.layersTitle}>LAYERS</h2>
      <div className={styles.layersSection}>
        <h3 className={styles.layersHeading}>PINS</h3>
        <Row layer="liveEvents" label="Live Events" icon={MAP_ICONS.layerLive} layers={layers} />
        <Row layer="pois" label="Points of interest" icon={MAP_ICONS.layerPoi} layers={layers} />
        <Row layer="pins" label="Quest Mini-Games" icon={MARKER_ART.quest} layers={layers} />
      </div>
      <hr className={styles.layersRule} />
      <div className={styles.layersSection}>
        <h3 className={styles.layersHeading}>DETAILS</h3>
        <Row layer="people" label="People" layers={layers} />
      </div>
      <hr className={styles.layersRule} />
      <div className={`${styles.layersSection} ${styles.layersMapType}`}>
        <h3 className={`${styles.layersHeading} ${styles.layersHeadingLarge}`}>MAP TYPE</h3>
        <div className={styles.mapTypes} role="radiogroup" aria-label="Map type">
          <MapType type="satellite" label="Satellite" icon={MAP_ICONS.satelliteView} layers={layers} />
          <MapType type="parcels" label="Parcels" icon={MAP_ICONS.parcelsView} layers={layers} />
        </div>
      </div>
    </section>
  )
}
