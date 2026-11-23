// Which layers the full-screen map shows. Held for the session only: a restart brings the
// defaults back.

export type MapLayer = 'liveEvents' | 'pois' | 'pins' | 'people' | 'satellite' | 'parcels'

const DEFAULT_LAYERS: readonly MapLayer[] = ['liveEvents', 'pois', 'pins', 'people', 'satellite']

export interface MapLayersStore {
  subscribe: (listener: () => void) => () => void
  getSnapshot: () => ReadonlySet<MapLayer>
  set: (layer: MapLayer, on: boolean) => void
  /** Show the satellite or the parcels map; exactly one is on. */
  setMapType: (type: 'satellite' | 'parcels') => void
}

export function createMapLayersStore(): MapLayersStore {
  let layers: ReadonlySet<MapLayer> = new Set(DEFAULT_LAYERS)
  const listeners = new Set<() => void>()
  const commit = (next: Set<MapLayer>): void => {
    layers = next
    listeners.forEach((l) => l())
  }
  return {
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getSnapshot: () => layers,
    set: (layer, on) => {
      if (layers.has(layer) === on) return
      const next = new Set(layers)
      if (on) next.add(layer)
      else next.delete(layer)
      commit(next)
    },
    setMapType: (type) => {
      if (layers.has(type) && !layers.has(type === 'satellite' ? 'parcels' : 'satellite')) return
      const next = new Set(layers)
      next.delete(type === 'satellite' ? 'parcels' : 'satellite')
      next.add(type)
      commit(next)
    }
  }
}

export const mapLayers = createMapLayersStore()
