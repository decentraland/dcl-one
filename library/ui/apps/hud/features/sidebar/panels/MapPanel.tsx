// MapPanel — what the navigation bar's Map button shows: the HUD's minimap, in a region named
// "Map". The minimap places itself beside the bar and keeps its own controls (collapse, zoom,
// settings, tip, the way back from a world), so this file draws no box of its own.

import type { ComponentProps } from 'react'
import { Minimap } from '../../minimap/Minimap'
import type { MapState, MinimapState } from '../../session/useEngineSession'
import styles from './MapPanel.module.css'

interface MapPanelProps {
  minimap: MinimapState
  map: MapState
  setEngineViewport: ComponentProps<typeof Minimap>['setEngineViewport']
}

export function MapPanel({ minimap, map, setEngineViewport }: MapPanelProps): React.JSX.Element {
  return (
    <section className={styles.root} aria-label="Map">
      <Minimap minimap={minimap} map={map} sceneTitle={minimap.sceneTitle} setEngineViewport={setEngineViewport} />
    </section>
  )
}
