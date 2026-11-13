// Where the lobby's four grids and their cells sit on the page's canvas (1920×1080 at its reference
// size, wider or taller as the window is): the top grids under the top bar beside the avatar, the
// bottom ones along the bottom edge, each pinned to its corner.

import { LOBBY_CORNERS, LOBBY_GRID_SIZES, type Cell, type LobbyCorner } from '@explorer-core/features/lobby/layout'

export const CELL_SIZE = 114
export const GUTTER = 2
const STRIDE = CELL_SIZE + GUTTER
const GRID_MARGIN = 40
const GRID_TOP = 97
const BOTTOM_MARGIN = 26

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

/** The pixels a span of cells covers, gutters between them included. */
export const spanSize = (cells: number): number => cells * CELL_SIZE + (cells - 1) * GUTTER

export function gridRect(corner: LobbyCorner, canvas: { width: number; height: number }): Rect {
  const size = LOBBY_GRID_SIZES[corner]
  const width = spanSize(size.x)
  const height = spanSize(size.y)
  const left = corner === 'TopLeft' || corner === 'BottomLeft'
  const top = corner === 'TopLeft' || corner === 'TopRight'
  return {
    x: left ? GRID_MARGIN : canvas.width - GRID_MARGIN - width,
    y: top ? GRID_TOP : canvas.height - BOTTOM_MARGIN - height,
    width,
    height
  }
}

export function cellRect(corner: LobbyCorner, cell: Cell, span: Cell, canvas: { width: number; height: number }): Rect {
  const grid = gridRect(corner, canvas)
  return { x: grid.x + cell.x * STRIDE, y: grid.y + cell.y * STRIDE, width: spanSize(span.x), height: spanSize(span.y) }
}

/** The cell nearest a point in a grid's own coordinates. */
export const nearestCell = (x: number, y: number): Cell => ({ x: Math.round(x / STRIDE), y: Math.round(y / STRIDE) })

/** The grid whose box holds the point, if any. */
export function gridAt(point: { x: number; y: number }, canvas: { width: number; height: number }): LobbyCorner | null {
  return (
    LOBBY_CORNERS.find((corner) => {
      const r = gridRect(corner, canvas)
      return point.x >= r.x && point.x <= r.x + r.width && point.y >= r.y && point.y <= r.y + r.height
    }) ?? null
  )
}
