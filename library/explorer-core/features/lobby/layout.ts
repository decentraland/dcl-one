// The lobby's widget layout: which widgets sit where on the four corner grids around the avatar.
// At most one placement per widget, none overlapping and all inside their grid. Cells are counted
// columns by rows from the grid's top-left corner. Pure data; the page draws it (LobbyHome) and
// the layout editor changes it.

export const LOBBY_CORNERS = ['TopLeft', 'TopRight', 'BottomLeft', 'BottomRight'] as const
export type LobbyCorner = (typeof LOBBY_CORNERS)[number]

/** Every widget, named with its span in cells, columns by rows: a resized widget gets a new id, so
 *  stored layouts drop it. */
export const LOBBY_WIDGETS = [
  'Landing4By3',
  'LiveEvents3By3',
  'UpcomingEvents3By2',
  'FriendsOnline4By2',
  'JumpBackIn7By2',
  'JumpBackIn4By2',
  'RecommendedPlaces7By2',
  'RecommendedPlaces4By2',
  'Communities7By2',
  'Communities4By2',
  'Gallery7By2',
  'Gallery4By2',
  'GoTo4By1'
] as const
export type LobbyWidgetId = (typeof LOBBY_WIDGETS)[number]

/** A widget as the player names it, whatever its size. A family with several sizes places one at a time. */
export const LOBBY_FAMILIES = ['Landing', 'LiveEvents', 'UpcomingEvents', 'FriendsOnline', 'JumpBackIn', 'RecommendedPlaces', 'Communities', 'Gallery', 'GoTo'] as const
export type LobbyWidgetFamily = (typeof LOBBY_FAMILIES)[number]

export interface Cell {
  x: number
  y: number
}

export interface LobbyPlacement {
  widget: LobbyWidgetId
  corner: LobbyCorner
  /** The cell under the widget's top-left corner. */
  cell: Cell
}

/** Grid sizes in cells, columns by rows. */
export type GridSizes = Record<LobbyCorner, Cell>

/** The lobby's grids at 1920×1080: four columns by six rows beside the avatar, seven by two below. */
export const LOBBY_GRID_SIZES: GridSizes = {
  TopLeft: { x: 4, y: 6 },
  TopRight: { x: 4, y: 6 },
  BottomLeft: { x: 7, y: 2 },
  BottomRight: { x: 7, y: 2 }
}

const SPANS: Record<LobbyWidgetId, Cell> = {
  Landing4By3: { x: 4, y: 3 },
  LiveEvents3By3: { x: 3, y: 3 },
  UpcomingEvents3By2: { x: 3, y: 2 },
  FriendsOnline4By2: { x: 4, y: 2 },
  JumpBackIn7By2: { x: 7, y: 2 },
  JumpBackIn4By2: { x: 4, y: 2 },
  RecommendedPlaces7By2: { x: 7, y: 2 },
  RecommendedPlaces4By2: { x: 4, y: 2 },
  Communities7By2: { x: 7, y: 2 },
  Communities4By2: { x: 4, y: 2 },
  Gallery7By2: { x: 7, y: 2 },
  Gallery4By2: { x: 4, y: 2 },
  GoTo4By1: { x: 4, y: 1 }
}

const CARD_COUNTS: Record<LobbyWidgetId, number> = {
  Landing4By3: 1,
  LiveEvents3By3: 1,
  UpcomingEvents3By2: 1,
  FriendsOnline4By2: 4,
  JumpBackIn7By2: 3,
  JumpBackIn4By2: 2,
  RecommendedPlaces7By2: 3,
  RecommendedPlaces4By2: 2,
  Communities7By2: 3,
  Communities4By2: 2,
  Gallery7By2: 3,
  Gallery4By2: 2,
  GoTo4By1: 1
}

/** A family's sizes, the first being the one it takes when none is picked. */
const VARIANTS: Record<LobbyWidgetFamily, readonly LobbyWidgetId[]> = {
  Landing: ['Landing4By3'],
  LiveEvents: ['LiveEvents3By3'],
  UpcomingEvents: ['UpcomingEvents3By2'],
  FriendsOnline: ['FriendsOnline4By2'],
  JumpBackIn: ['JumpBackIn7By2', 'JumpBackIn4By2'],
  RecommendedPlaces: ['RecommendedPlaces7By2', 'RecommendedPlaces4By2'],
  Communities: ['Communities7By2', 'Communities4By2'],
  Gallery: ['Gallery7By2', 'Gallery4By2'],
  GoTo: ['GoTo4By1']
}

const TITLES: Record<LobbyWidgetFamily, string> = {
  Landing: 'Jump In Home',
  LiveEvents: 'Live Events',
  UpcomingEvents: 'Upcoming Events',
  FriendsOnline: 'Friends',
  JumpBackIn: 'Jump Back In',
  RecommendedPlaces: 'Recommended Places',
  Communities: 'My Communities',
  Gallery: 'Gallery',
  GoTo: 'Go to'
}

export const widgetSpan = (widget: LobbyWidgetId): Cell => SPANS[widget]
/** How many cards the widget shows at once. */
export const widgetCardCount = (widget: LobbyWidgetId): number => CARD_COUNTS[widget]
export const familyVariants = (family: LobbyWidgetFamily): readonly LobbyWidgetId[] => VARIANTS[family]
export const familyTitle = (family: LobbyWidgetFamily): string => TITLES[family]
export function widgetFamily(widget: LobbyWidgetId): LobbyWidgetFamily {
  return LOBBY_FAMILIES.find((f) => VARIANTS[f].includes(widget)) as LobbyWidgetFamily
}

const DEFAULT_PLACEMENTS: readonly LobbyPlacement[] = [
  { widget: 'Landing4By3', corner: 'TopLeft', cell: { x: 0, y: 0 } },
  { widget: 'FriendsOnline4By2', corner: 'TopLeft', cell: { x: 0, y: 3 } },
  { widget: 'LiveEvents3By3', corner: 'TopRight', cell: { x: 1, y: 0 } },
  { widget: 'UpcomingEvents3By2', corner: 'TopRight', cell: { x: 1, y: 3 } },
  { widget: 'JumpBackIn7By2', corner: 'BottomLeft', cell: { x: 0, y: 0 } },
  { widget: 'RecommendedPlaces7By2', corner: 'BottomRight', cell: { x: 0, y: 0 } }
]

/** Down, right, up and left; the order breaks ties between equally cheap pushes. */
const PUSH_DIRECTIONS: readonly Cell[] = [
  { x: 0, y: 1 },
  { x: 1, y: 0 },
  { x: 0, y: -1 },
  { x: -1, y: 0 }
]

const fits = (grid: Cell, cell: Cell, span: Cell): boolean => cell.x >= 0 && cell.y >= 0 && cell.x + span.x <= grid.x && cell.y + span.y <= grid.y

const overlap = (a: Cell, aSpan: Cell, b: Cell, bSpan: Cell): boolean =>
  a.x < b.x + bSpan.x && b.x < a.x + aSpan.x && a.y < b.y + bSpan.y && b.y < a.y + aSpan.y

/** How many cells the widget must travel in the direction to clear the pusher it overlaps. */
function displacement(direction: Cell, pusher: Cell, pusherSpan: Cell, other: Cell, otherSpan: Cell): number {
  if (direction.y > 0) return pusher.y + pusherSpan.y - other.y
  if (direction.y < 0) return other.y + otherSpan.y - pusher.y
  if (direction.x > 0) return pusher.x + pusherSpan.x - other.x
  return other.x + otherSpan.x - pusher.x
}

export class LobbyLayout {
  private readonly placements: LobbyPlacement[] = []

  constructor(private readonly grids: GridSizes = LOBBY_GRID_SIZES) {}

  get all(): readonly LobbyPlacement[] {
    return this.placements
  }

  gridSize(corner: LobbyCorner): Cell {
    return this.grids[corner]
  }

  placement(widget: LobbyWidgetId): LobbyPlacement | undefined {
    return this.placements.find((p) => p.widget === widget)
  }

  clone(): LobbyLayout {
    const copy = new LobbyLayout(this.grids)
    copy.placements.push(...this.placements.map((p) => ({ ...p, cell: { ...p.cell } })))
    return copy
  }

  equals(other: LobbyLayout): boolean {
    if (other.placements.length !== this.placements.length) return false
    return this.placements.every((p) => {
      const o = other.placement(p.widget)
      return o != null && o.corner === p.corner && o.cell.x === p.cell.x && o.cell.y === p.cell.y
    })
  }

  /** Places the widget, moving it if already placed; fails when the cell doesn't fit the grid or overlaps
   *  another widget, leaving the layout as it was. */
  tryPlace(widget: LobbyWidgetId, corner: LobbyCorner, cell: Cell): boolean {
    if (!this.isFree(corner, cell, widgetSpan(widget), widget)) return false
    this.remove(widget)
    this.placements.push({ widget, corner, cell: { ...cell } })
    return true
  }

  /** Places the widget like tryPlace, but the widgets in its way move aside as resolvePush works out. Fails
   *  the same way, leaving the layout as it was. */
  tryPlacePushing(widget: LobbyWidgetId, corner: LobbyCorner, cell: Cell): boolean {
    const pushed = this.resolvePush(widget, corner, cell)
    if (pushed == null) return false
    for (const p of pushed) {
      const i = this.placements.findIndex((q) => q.widget === p.widget)
      this.placements[i] = p
    }
    this.remove(widget)
    this.placements.push({ widget, corner, cell: { ...cell } })
    return true
  }

  remove(widget: LobbyWidgetId): boolean {
    const i = this.placements.findIndex((p) => p.widget === widget)
    if (i < 0) return false
    this.placements.splice(i, 1)
    return true
  }

  clear(): void {
    this.placements.length = 0
  }

  /** The placements the lobby ships with. */
  resetToDefault(): void {
    this.clear()
    for (const p of DEFAULT_PLACEMENTS) this.tryPlace(p.widget, p.corner, p.cell)
  }

  /** Whether a widget of the span fits the grid at the cell without overlapping any placed widget other than `ignoring`. */
  isFree(corner: LobbyCorner, cell: Cell, span: Cell, ignoring?: LobbyWidgetId): boolean {
    if (!fits(this.grids[corner], cell, span)) return false
    return !this.placements.some((o) => o.corner === corner && o.widget !== ignoring && overlap(cell, span, o.cell, widgetSpan(o.widget)))
  }

  /**
   * Where the grid's widgets end up if the widget takes the cell and whatever it overlaps moves off the side it
   * covers least, by as many cells as clear it. A side the grid's edge, the placed widget or a widget this
   * resolution already moved blocks is passed over for the next cheapest. Every moved widget pushes in turn
   * whatever it lands on, and moves only once. Returns the new placements of the widgets that move (none when the
   * cell is free), or null when the widget doesn't fit the grid or one in the way has no side left. The layout
   * itself doesn't change.
   */
  resolvePush(widget: LobbyWidgetId, corner: LobbyCorner, cell: Cell): LobbyPlacement[] | null {
    const grid = this.grids[corner]
    const span = widgetSpan(widget)
    if (!fits(grid, cell, span)) return null

    const pushed = this.placements.filter((p) => p.corner === corner && p.widget !== widget).map((p) => ({ ...p, cell: { ...p.cell } }))
    const moved = new Set<number>()
    // the placed widget pushes first (-1); every widget it pushes queues to push in turn
    const queue: number[] = [-1]
    for (let head = 0; head < queue.length; head++) {
      const pusher = queue[head]
      const pusherCell = pusher < 0 ? cell : pushed[pusher].cell
      const pusherSpan = pusher < 0 ? span : widgetSpan(pushed[pusher].widget)
      for (let i = 0; i < pushed.length; i++) {
        if (i === pusher) continue
        const other = pushed[i]
        const otherSpan = widgetSpan(other.widget)
        if (!overlap(pusherCell, pusherSpan, other.cell, otherSpan)) continue
        const to = moved.has(i) ? null : this.pushAway(grid, cell, span, pushed, moved, i, pusherCell, pusherSpan)
        if (to == null) return null
        pushed[i] = { widget: other.widget, corner, cell: to }
        moved.add(i)
        queue.push(i)
      }
    }
    return pushed.filter((_, i) => moved.has(i))
  }

  /** The free cell closest to the preferred position (fractional and possibly outside the grid): rounded and
   *  clamped to where the span fits, then searched outwards by Manhattan distance. */
  findSnapCell(corner: LobbyCorner, span: Cell, preferred: { x: number; y: number }): Cell | null {
    const grid = this.grids[corner]
    const maxX = grid.x - span.x
    const maxY = grid.y - span.y
    if (maxX < 0 || maxY < 0) return null
    const ox = Math.min(Math.max(Math.floor(preferred.x + 0.5), 0), maxX)
    const oy = Math.min(Math.max(Math.floor(preferred.y + 0.5), 0), maxY)
    for (let distance = 0; distance <= maxX + maxY; distance++) {
      for (let dx = -distance; dx <= distance; dx++) {
        const dy = distance - Math.abs(dx)
        const down = { x: ox + dx, y: oy + dy }
        if (this.isFree(corner, down, span)) return down
        if (dy === 0) continue
        const up = { x: ox + dx, y: oy - dy }
        if (this.isFree(corner, up, span)) return up
      }
    }
    return null
  }

  // The cheapest side of the pusher the widget at `index` can move off to: the fewest cells among those where it
  // fits the grid and overlaps neither the placed widget nor a moved one.
  private pushAway(grid: Cell, placedCell: Cell, placedSpan: Cell, pushed: LobbyPlacement[], moved: Set<number>, index: number, pusherCell: Cell, pusherSpan: Cell): Cell | null {
    const otherCell = pushed[index].cell
    const otherSpan = widgetSpan(pushed[index].widget)
    let best = Infinity
    let to: Cell | null = null
    for (const direction of PUSH_DIRECTIONS) {
      const d = displacement(direction, pusherCell, pusherSpan, otherCell, otherSpan)
      if (d >= best) continue
      const candidate = { x: otherCell.x + direction.x * d, y: otherCell.y + direction.y * d }
      if (!fits(grid, candidate, otherSpan) || overlap(candidate, otherSpan, placedCell, placedSpan)) continue
      if (pushed.some((p, i) => i !== index && moved.has(i) && overlap(candidate, otherSpan, p.cell, widgetSpan(p.widget)))) continue
      best = d
      to = candidate
    }
    return to
  }
}
