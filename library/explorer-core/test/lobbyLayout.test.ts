import { beforeEach, describe, expect, it } from 'vitest'
import { LobbyLayout, type Cell, type GridSizes, type LobbyPlacement, type LobbyWidgetId, widgetFamily, familyVariants, widgetSpan } from '../features/lobby/layout'
import { deserializeLayout, serializeLayout } from '../features/lobby/layoutStorage'

const TOP: Cell = { x: 4, y: 6 }
const BOTTOM: Cell = { x: 7, y: 2 }
const grids = (topLeft: Cell = TOP, bottomLeft: Cell = BOTTOM): GridSizes => ({ TopLeft: topLeft, TopRight: TOP, BottomLeft: bottomLeft, BottomRight: BOTTOM })
const cell = (x: number, y: number): Cell => ({ x, y })

let layout: LobbyLayout
beforeEach(() => {
  layout = new LobbyLayout(grids())
})

function pushedCell(pushed: LobbyPlacement[] | null, widget: LobbyWidgetId): Cell {
  const p = pushed?.find((q) => q.widget === widget)
  if (p == null) throw new Error(`${widget} was not pushed`)
  return p.cell
}

describe('LobbyLayout placements', () => {
  it('places a widget inside its grid', () => {
    expect(layout.tryPlace('LiveEvents3By3', 'TopRight', cell(1, 2))).toBe(true)
    expect(layout.placement('LiveEvents3By3')).toEqual({ widget: 'LiveEvents3By3', corner: 'TopRight', cell: cell(1, 2) })
    expect(layout.all).toHaveLength(1)
  })

  it.each([
    ['past the columns', 2, 0],
    ['past the rows', 0, 4],
    ['before the columns', -1, 0],
    ['before the rows', 0, -1]
  ])('rejects a placement %s', (_, x, y) => {
    expect(layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(x, y))).toBe(false)
    expect(layout.all).toHaveLength(0)
  })

  it('rejects an overlapping placement', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    expect(layout.tryPlace('FriendsOnline4By2', 'TopLeft', cell(0, 2))).toBe(false)
    expect(layout.placement('FriendsOnline4By2')).toBeUndefined()
  })

  it('allows the same cells on another grid', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    expect(layout.tryPlace('FriendsOnline4By2', 'TopRight', cell(0, 0))).toBe(true)
  })

  it('moves a placed widget over its own cells', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    expect(layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 1))).toBe(true)
    expect(layout.all).toHaveLength(1)
    expect(layout.all[0].cell).toEqual(cell(0, 1))
  })

  it('keeps the old placement when a move fails', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    layout.tryPlace('FriendsOnline4By2', 'TopLeft', cell(0, 4))
    expect(layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 2))).toBe(false)
    expect(layout.placement('Landing4By3')?.cell).toEqual(cell(0, 0))
  })

  it('removes a placement', () => {
    layout.tryPlace('JumpBackIn7By2', 'BottomLeft', cell(0, 0))
    expect(layout.remove('JumpBackIn7By2')).toBe(true)
    expect(layout.remove('JumpBackIn7By2')).toBe(false)
    expect(layout.all).toHaveLength(0)
    expect(layout.isFree('BottomLeft', cell(0, 0), widgetSpan('JumpBackIn7By2'))).toBe(true)
  })

  it('reports occupied cells as not free', () => {
    layout.tryPlace('LiveEvents3By3', 'TopRight', cell(1, 0))
    expect(layout.isFree('TopRight', cell(0, 2), cell(2, 1))).toBe(false)
    expect(layout.isFree('TopRight', cell(0, 0), cell(1, 6))).toBe(true)
    expect(layout.isFree('TopRight', cell(1, 3), cell(3, 3))).toBe(true)
  })

  it('ignores a widget when asked', () => {
    layout.tryPlace('LiveEvents3By3', 'TopRight', cell(1, 0))
    expect(layout.isFree('TopRight', cell(0, 0), cell(4, 2))).toBe(false)
    expect(layout.isFree('TopRight', cell(0, 0), cell(4, 2), 'LiveEvents3By3')).toBe(true)
  })
})

describe('LobbyLayout pushes', () => {
  it('pushes the widget down when covering its top rows, changing nothing', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 0))
    expect(pushed).toEqual([{ widget: 'Landing4By3', corner: 'TopLeft', cell: cell(0, 2) }])
    expect(layout.placement('Landing4By3')?.cell).toEqual(cell(0, 0))
    expect(layout.placement('FriendsOnline4By2')).toBeUndefined()
  })

  it('pushes the widget right when covering its left columns', () => {
    layout = new LobbyLayout(grids(cell(8, 6)))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(3, 0))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 0))
    expect(pushed).toHaveLength(1)
    expect(pushedCell(pushed, 'UpcomingEvents3By2')).toEqual(cell(4, 0))
  })

  it('pushes the widget left when covering its right columns', () => {
    layout = new LobbyLayout(grids(cell(8, 6)))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(1, 0))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(3, 0))
    expect(pushed).toHaveLength(1)
    expect(pushedCell(pushed, 'UpcomingEvents3By2')).toEqual(cell(0, 0))
  })

  it('pushes the widget up when covering its bottom rows', () => {
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(0, 2))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 3))
    expect(pushed).toHaveLength(1)
    expect(pushedCell(pushed, 'UpcomingEvents3By2')).toEqual(cell(0, 1))
  })

  it('pushes off the next cheapest side when the grid blocks the cheapest', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 1))
    // up would leave the grid, so the widget goes down
    expect(pushedCell(pushed, 'Landing4By3')).toEqual(cell(0, 3))
  })

  it('pushes sideways on a grid too short to push down', () => {
    layout = new LobbyLayout(grids(TOP, cell(6, 2)))
    layout.tryPlace('UpcomingEvents3By2', 'BottomLeft', cell(0, 0))
    // right would leave a six-column grid and down a two-row one
    expect(layout.resolvePush('FriendsOnline4By2', 'BottomLeft', cell(0, 0))).toBeNull()

    layout = new LobbyLayout(grids())
    layout.tryPlace('UpcomingEvents3By2', 'BottomLeft', cell(0, 0))
    expect(pushedCell(layout.resolvePush('FriendsOnline4By2', 'BottomLeft', cell(0, 0)), 'UpcomingEvents3By2')).toEqual(cell(4, 0))
  })

  it('pushes a chained widget off the side of the widget that lands on it', () => {
    layout = new LobbyLayout(grids(cell(9, 6)))
    layout.tryPlace('Landing4By3', 'TopRight', cell(0, 0))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(3, 1))
    layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(6, 0))
    const pushed = layout.resolvePush('Landing4By3', 'TopLeft', cell(0, 0))
    expect(pushed).toHaveLength(2)
    expect(pushedCell(pushed, 'UpcomingEvents3By2')).toEqual(cell(4, 1))
    // down off the pushed widget: right would leave the grid
    expect(pushedCell(pushed, 'LiveEvents3By3')).toEqual(cell(6, 3))
  })

  it('fails when a chained push can only land on the placed widget', () => {
    layout = new LobbyLayout(grids(cell(9, 4)))
    layout.tryPlace('Landing4By3', 'TopRight', cell(0, 0))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(3, 1))
    layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(6, 0))
    expect(layout.resolvePush('Landing4By3', 'TopLeft', cell(0, 0))).toBeNull()
  })

  it('pushes the widgets a pushed widget lands on', () => {
    layout = new LobbyLayout(grids(cell(4, 8)))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(0, 0))
    layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(0, 2))
    const pushed = layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 0))
    expect(pushed).toHaveLength(2)
    expect(pushedCell(pushed, 'UpcomingEvents3By2')).toEqual(cell(0, 2))
    expect(pushedCell(pushed, 'LiveEvents3By3')).toEqual(cell(0, 4))
  })

  it('pushes only the widgets in the way', () => {
    layout = new LobbyLayout(grids(cell(7, 8)))
    layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(0, 0))
    layout.tryPlace('UpcomingEvents3By2', 'TopLeft', cell(4, 0))
    layout.tryPlace('FriendsOnline4By2', 'TopLeft', cell(0, 6))
    layout.tryPlace('JumpBackIn7By2', 'BottomLeft', cell(0, 0))
    const pushed = layout.resolvePush('Landing4By3', 'TopLeft', cell(0, 0))
    expect(pushed).toHaveLength(1)
    expect(pushedCell(pushed, 'LiveEvents3By3')).toEqual(cell(0, 3))
  })

  it('resolves a free cell without pushes', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    expect(layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 3))).toEqual([])
  })

  it('fails to push a widget off the grid', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 3))
    expect(layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(0, 2))).toBeNull()
  })

  it('fails to push from outside the grid', () => {
    expect(layout.resolvePush('FriendsOnline4By2', 'TopLeft', cell(1, 0))).toBeNull()
  })

  it('moves the widgets in the way when placing with a push', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    layout.tryPlace('FriendsOnline4By2', 'TopLeft', cell(0, 3))
    expect(layout.tryPlacePushing('FriendsOnline4By2', 'TopLeft', cell(0, 0))).toBe(true)
    expect(layout.all).toHaveLength(2)
    expect(layout.placement('FriendsOnline4By2')?.cell).toEqual(cell(0, 0))
    // the pushed widget takes the cells the moved one leaves
    expect(layout.placement('Landing4By3')?.cell).toEqual(cell(0, 2))
  })

  it('pushes when moving a widget in from another grid', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    layout.tryPlace('LiveEvents3By3', 'TopRight', cell(1, 0))
    expect(layout.tryPlacePushing('LiveEvents3By3', 'TopLeft', cell(0, 0))).toBe(true)
    expect(layout.placement('LiveEvents3By3')).toEqual({ widget: 'LiveEvents3By3', corner: 'TopLeft', cell: cell(0, 0) })
    expect(layout.placement('Landing4By3')?.cell).toEqual(cell(0, 3))
  })

  it('keeps the layout when a push fails', () => {
    layout.tryPlace('FriendsOnline4By2', 'TopLeft', cell(0, 0))
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 3))
    expect(layout.tryPlacePushing('FriendsOnline4By2', 'TopLeft', cell(0, 2))).toBe(false)
    expect(layout.placement('FriendsOnline4By2')?.cell).toEqual(cell(0, 0))
    expect(layout.placement('Landing4By3')?.cell).toEqual(cell(0, 3))
  })
})

describe('LobbyLayout snapping', () => {
  it('snaps to the nearest cell', () => {
    expect(layout.findSnapCell('TopLeft', cell(2, 2), { x: 1.4, y: 2.6 })).toEqual(cell(1, 3))
  })

  it('snaps to the closest free cell when the nearest is taken', () => {
    layout.tryPlace('Landing4By3', 'TopLeft', cell(0, 0))
    expect(layout.findSnapCell('TopLeft', widgetSpan('FriendsOnline4By2'), { x: 0, y: 2 })).toEqual(cell(0, 3))
  })

  it('clamps the snap cell to where the span fits', () => {
    expect(layout.findSnapCell('BottomRight', cell(3, 1), { x: 9, y: 7 })).toEqual(cell(4, 1))
  })

  it('clamps a negative snap position to the first cell', () => {
    expect(layout.findSnapCell('TopLeft', cell(1, 1), { x: -3, y: -0.7 })).toEqual(cell(0, 0))
  })

  it('fails to snap when the span is larger than the grid', () => {
    expect(layout.findSnapCell('BottomLeft', widgetSpan('LiveEvents3By3'), { x: 0, y: 0 })).toBeNull()
  })

  it('fails to snap when the grid is full', () => {
    layout.tryPlace('JumpBackIn7By2', 'BottomLeft', cell(0, 0))
    expect(layout.findSnapCell('BottomLeft', cell(1, 1), { x: 2, y: 1 })).toBeNull()
  })
})

describe('LobbyLayout defaults', () => {
  it('places every widget but the four-column places by default', () => {
    layout.resetToDefault()
    expect(layout.all).toHaveLength(6)
    expect(layout.placement('JumpBackIn4By2')).toBeUndefined()
    expect(layout.placement('RecommendedPlaces4By2')).toBeUndefined()
    expect(layout.placement('Landing4By3')?.corner).toBe('TopLeft')
    expect(layout.placement('FriendsOnline4By2')?.corner).toBe('TopLeft')
    expect(layout.placement('LiveEvents3By3')?.corner).toBe('TopRight')
    expect(layout.placement('UpcomingEvents3By2')?.corner).toBe('TopRight')
    expect(layout.placement('JumpBackIn7By2')?.corner).toBe('BottomLeft')
    expect(layout.placement('RecommendedPlaces7By2')?.corner).toBe('BottomRight')
  })

  it('replaces the placements with the default', () => {
    layout.tryPlace('Landing4By3', 'BottomLeft', cell(0, 0))
    layout.resetToDefault()
    expect(layout.placement('Landing4By3')?.corner).toBe('TopLeft')
  })

  it('clears every placement', () => {
    layout.resetToDefault()
    layout.clear()
    expect(layout.all).toHaveLength(0)
  })

  it('exposes the grid sizes', () => {
    expect(layout.gridSize('TopLeft')).toEqual(TOP)
    expect(layout.gridSize('TopRight')).toEqual(TOP)
    expect(layout.gridSize('BottomLeft')).toEqual(BOTTOM)
    expect(layout.gridSize('BottomRight')).toEqual(BOTTOM)
  })

  it('names each size with its family, the default size first', () => {
    expect(widgetFamily('JumpBackIn4By2')).toBe('JumpBackIn')
    expect(familyVariants('Gallery')).toEqual(['Gallery7By2', 'Gallery4By2'])
  })
})

describe('lobby layout storage', () => {
  const defaults = (): LobbyLayout => {
    const l = new LobbyLayout(grids())
    l.resetToDefault()
    return l
  }

  it('uses the default layout when nothing is stored', () => {
    deserializeLayout(layout, null)
    expect(layout.equals(defaults())).toBe(true)
  })

  it('round-trips the placements', () => {
    layout.tryPlace('LiveEvents3By3', 'TopLeft', cell(0, 1))
    layout.tryPlace('Landing4By3', 'TopRight', cell(0, 2))
    layout.tryPlace('RecommendedPlaces7By2', 'BottomLeft', cell(0, 0))
    const loaded = new LobbyLayout(grids())
    deserializeLayout(loaded, serializeLayout(layout))
    expect(loaded.equals(layout)).toBe(true)
  })

  it('loads a stored empty layout as empty', () => {
    const stored = serializeLayout(layout)
    layout.resetToDefault()
    deserializeLayout(layout, stored)
    expect(layout.all).toHaveLength(0)
  })

  it('skips unknown widgets and corners', () => {
    deserializeLayout(
      layout,
      '{"Version":1,"Entries":[' +
        '{"Corner":"TopLeft","Widget":"Landing9By9","X":0,"Y":0},' +
        '{"Corner":"Middle","Widget":"LiveEvents3By3","X":0,"Y":0},' +
        '{"Corner":"7","Widget":"Landing4By3","X":0,"Y":0},' +
        '{"Corner":"TopLeft","Widget":"42","X":0,"Y":0},' +
        '{"Corner":"BottomRight","Widget":"JumpBackIn7By2","X":0,"Y":0}]}'
    )
    expect(layout.all).toHaveLength(1)
    expect(layout.placement('JumpBackIn7By2')?.corner).toBe('BottomRight')
  })

  it('skips entries that do not fit the grid', () => {
    deserializeLayout(
      layout,
      '{"Version":1,"Entries":[' +
        '{"Corner":"TopLeft","Widget":"Landing4By3","X":0,"Y":0},' +
        '{"Corner":"TopLeft","Widget":"FriendsOnline4By2","X":0,"Y":2},' +
        '{"Corner":"BottomLeft","Widget":"LiveEvents3By3","X":0,"Y":0}]}'
    )
    expect(layout.all.map((p) => p.widget)).toEqual(['Landing4By3'])
  })

  it('falls back to the default on malformed JSON', () => {
    deserializeLayout(layout, '{"Version":1,"Entries":[{')
    expect(layout.equals(defaults())).toBe(true)
  })

  it('falls back to the default on another version', () => {
    deserializeLayout(layout, '{"Version":0,"Entries":[]}')
    expect(layout.equals(defaults())).toBe(true)
  })

  it('stores widgets and corners by name', () => {
    layout.tryPlace('LiveEvents3By3', 'TopRight', cell(1, 0))
    const json = serializeLayout(layout)
    expect(json).toContain('"Widget":"LiveEvents3By3"')
    expect(json).toContain('"Corner":"TopRight"')
  })
})
