import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  COLLAPSED_GROUPS_KEY,
  allCollapsed,
  expandGroup,
  readCollapsedGroups,
  toggleAllGroups,
  toggleGroup,
  writeCollapsedGroups
} from '../src/shell/collapsedGroups'

const known = ['foundations', 'component:Actions', 'hud']

beforeEach(() => window.localStorage.clear())
afterEach(() => vi.restoreAllMocks())

describe('when the collapsed groups are read from the browser', () => {
  it('should return what an earlier visit stored', () => {
    writeCollapsedGroups(new Set(['hud', 'foundations']))
    expect([...readCollapsedGroups(known)]).toEqual(['foundations', 'hud'])
  })

  it('should start with every group expanded when nothing is stored', () => {
    expect(readCollapsedGroups(known).size).toBe(0)
  })

  it('should drop a stored group that no longer exists', () => {
    window.localStorage.setItem(COLLAPSED_GROUPS_KEY, JSON.stringify(['component:Gone', 'hud']))
    expect([...readCollapsedGroups(known)]).toEqual(['hud'])
  })

  it.each(['not json', '{"hud":true}', '"hud"', 'null', '[1,null,{"id":"hud"}]'])(
    'should start with every group expanded when storage holds %s',
    (stored) => {
      window.localStorage.setItem(COLLAPSED_GROUPS_KEY, stored)
      expect(readCollapsedGroups(known).size).toBe(0)
    }
  )

  it('should start with every group expanded when reading storage throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    expect(readCollapsedGroups(known).size).toBe(0)
  })

  it('should start with every group expanded when the browser withholds storage itself', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    expect(readCollapsedGroups(known).size).toBe(0)
    expect(() => writeCollapsedGroups(new Set(['hud']))).not.toThrow()
  })
})

describe('when the collapsed groups are kept for the next visit', () => {
  it('should not throw when the browser refuses the write, and leave what was stored', () => {
    writeCollapsedGroups(new Set(['hud']))
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new DOMException('full', 'QuotaExceededError')
    })
    expect(() => writeCollapsedGroups(new Set(['foundations']))).not.toThrow()
    vi.restoreAllMocks()
    expect([...readCollapsedGroups(known)]).toEqual(['hud'])
  })
})

describe('when a group is toggled', () => {
  it('should collapse an expanded group and expand a collapsed one, leaving the others', () => {
    const collapsed = toggleGroup(new Set(['hud']), 'foundations')
    expect([...collapsed]).toEqual(['hud', 'foundations'])
    expect([...toggleGroup(collapsed, 'hud')]).toEqual(['foundations'])
  })

  it('should expand only a collapsed group, and return the same set otherwise', () => {
    const collapsed: ReadonlySet<string> = new Set(['hud'])
    expect(expandGroup(collapsed, 'foundations')).toBe(collapsed)
    expect([...expandGroup(collapsed, 'hud')]).toEqual([])
    expect([...collapsed]).toEqual(['hud'])
  })
})

describe('when every group is toggled at once', () => {
  it('should collapse them all when none is collapsed', () => {
    expect([...toggleAllGroups(new Set(), known)]).toEqual(known)
  })

  it('should collapse the rest when one group is already collapsed', () => {
    expect([...toggleAllGroups(new Set(['hud']), known)]).toEqual(known)
  })

  it('should expand them all only when every one is collapsed', () => {
    expect(allCollapsed(new Set(known), known)).toBe(true)
    expect(toggleAllGroups(new Set(known), known).size).toBe(0)
    expect(allCollapsed(new Set(['hud', 'foundations']), known)).toBe(false)
  })

  it('should not call an empty sidebar collapsed', () => {
    expect(allCollapsed(new Set(), [])).toBe(false)
  })
})
