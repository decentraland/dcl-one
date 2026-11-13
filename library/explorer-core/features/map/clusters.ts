// Marker clustering for the full-screen map. Markers are bucketed by their key parcel into a
// grid whose cell size depends on the zoom level; a cell holding more than one marker shows as
// a single counted cluster at their average position.

import { PARCEL_UNITS, type Vec2 } from './mapCoords'

// Cell size, in parcels, for each zoom level (0 = farthest).
const ZOOM_TO_CELL_SIZE = [35, 25, 27, 12, 7, 4, 1]

export function clusterCellSize(zoomLevel: number): number {
  return ZOOM_TO_CELL_SIZE[zoomLevel] ?? 1
}

export interface Clusterable {
  /** The parcel the marker is keyed by. */
  key: Vec2
  position: Vec2
}

export interface Cluster {
  id: string
  count: number
  position: Vec2
}

export interface Clustered<T> {
  visible: T[]
  clusters: Cluster[]
}

export function clusterMarkers<T extends Clusterable>(markers: readonly T[], zoomLevel: number): Clustered<T> {
  const size = clusterCellSize(zoomLevel)
  const grid = new Map<string, T[]>()
  for (const marker of markers) {
    const cell = `${Math.floor(marker.key.x / size)},${Math.floor(marker.key.y / size)}`
    const bucket = grid.get(cell)
    if (bucket) bucket.push(marker)
    else grid.set(cell, [marker])
  }

  const visible: T[] = []
  const clusters: Cluster[] = []
  for (const [cell, bucket] of grid) {
    if (bucket.length === 1) {
      visible.push(bucket[0])
      continue
    }
    let x = 0
    let y = 0
    for (const marker of bucket) {
      x += marker.position.x
      y += marker.position.y
    }
    // the cluster takes a half-parcel pivot offset on top of positions that already carry it
    clusters.push({
      id: cell,
      count: bucket.length,
      position: { x: x / bucket.length - PARCEL_UNITS / 2, y: y / bucket.length - PARCEL_UNITS / 2 }
    })
  }
  return { visible, clusters }
}
