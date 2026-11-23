import { describe, expect, it } from 'vitest'
import {
  canvasHref,
  galleryHref,
  generatedHref,
  pageAliases,
  parseRoute,
  parseWidth,
  widthPresets
} from '../src/url'

const components = ['Button', 'NavBar']
const foundations = ['colors', 'icons']

describe('when the gallery reads its URL', () => {
  it('should open the requested runtime component at the requested width', () => {
    expect(parseRoute('?component=NavBar&width=1024', components, foundations)).toEqual({
      source: 'runtime',
      view: 'component',
      name: 'NavBar',
      width: 1024
    })
  })

  it('should show the first page and name an unknown component instead of failing', () => {
    expect(parseRoute('?component=Nope', components, foundations)).toEqual({
      source: 'runtime',
      view: 'component',
      name: 'Button',
      width: 'fill',
      missing: 'Nope'
    })
    expect(parseRoute('', components, foundations)).toEqual({
      source: 'runtime',
      view: 'component',
      name: 'Button',
      width: 'fill'
    })
  })

  it('should open foundations pages and name an unknown one', () => {
    expect(parseRoute('?foundations=icons', components, foundations)).toMatchObject({
      view: 'foundations',
      name: 'icons'
    })
    expect(parseRoute('?foundations=', components, foundations)).toMatchObject({
      name: 'colors',
      missing: ''
    })
  })

  it('should forward every generated Review parameter except the source', () => {
    const route = parseRoute(
      '?source=generated&component=NavBar&variant=3&audit=1',
      components,
      foundations
    )
    expect(route).toEqual({ source: 'generated', query: 'component=NavBar&variant=3&audit=1' })
    expect(generatedHref('../dcl-ui-review/', 'component=NavBar&variant=3')).toBe(
      '../dcl-ui-review/?component=NavBar&variant=3'
    )
    expect(generatedHref('../dcl-ui-review/', '')).toBe('../dcl-ui-review/')
  })
})

describe('when an export shown on another page is requested by its own name', () => {
  const pages = ['Button', 'EventCard']
  const aliases = { EventCardHost: 'EventCard', EventCardAction: 'EventCard' }

  it('should open the owning page and keep the width', () => {
    expect(parseRoute('?component=EventCardHost&width=375', pages, foundations, aliases)).toEqual({
      source: 'runtime',
      view: 'component',
      name: 'EventCard',
      width: 375,
      alias: 'EventCardHost'
    })
  })

  it('should prefer a page of that name over an alias', () => {
    expect(parseRoute('?component=EventCard', pages, foundations, { EventCard: 'Button' })).toEqual(
      { source: 'runtime', view: 'component', name: 'EventCard', width: 'fill' }
    )
  })

  it('should still name an unknown export as missing, and ignore inherited object keys', () => {
    for (const unknown of ['EventCardNope', 'constructor', 'toString'])
      expect(parseRoute(`?component=${unknown}`, pages, foundations, aliases)).toEqual({
        source: 'runtime',
        view: 'component',
        name: 'Button',
        width: 'fill',
        missing: unknown
      })
  })
})

describe('when a section of the HUD Showcase is requested', () => {
  const hud = { pages: ['Showcase', 'Button', 'MenuPanel'], aliases: { MenuRow: 'MenuPanel' } }

  it('should open the section at the requested width, in the HUD document', () => {
    expect(parseRoute('?hud=MenuPanel&width=1280', components, foundations, {}, hud)).toEqual({
      source: 'runtime',
      view: 'hud',
      name: 'MenuPanel',
      width: 1280
    })
    expect(canvasHref('hud', 'MenuPanel')).toBe('hud.html?hud=MenuPanel')
    expect(canvasHref('component', 'Button')).toBe('frame.html?component=Button')
  })

  it('should keep a HUD section apart from the runtime page of the same name', () => {
    expect(parseRoute('?hud=Button', components, foundations, {}, hud)).toMatchObject({
      view: 'hud',
      name: 'Button'
    })
    expect(parseRoute('?component=Button', components, foundations, {}, hud)).toMatchObject({
      view: 'component',
      name: 'Button'
    })
  })

  it('should open a component on the section that renders it', () => {
    expect(parseRoute('?hud=MenuRow&width=375', components, foundations, {}, hud)).toEqual({
      source: 'runtime',
      view: 'hud',
      name: 'MenuPanel',
      width: 375,
      alias: 'MenuRow'
    })
  })

  it('should show the Showcase and name an unknown, empty or inherited section', () => {
    for (const unknown of ['Nope', '', 'constructor'])
      expect(parseRoute(`?hud=${unknown}`, components, foundations, {}, hud)).toEqual({
        source: 'runtime',
        view: 'hud',
        name: 'Showcase',
        width: 'fill',
        missing: unknown
      })
  })
})

describe('when pages show exports named after other pages', () => {
  it('should open each export on the first page that lists it, and never a page on another', () => {
    expect(
      pageAliases([
        { name: 'MenuPanel', components: ['MenuPanel', 'MenuRow', 'Icon'] },
        { name: 'IconButton', components: ['IconButton', 'Icon', 'MenuPanel'] }
      ])
    ).toEqual({ MenuRow: 'MenuPanel', Icon: 'MenuPanel' })
    expect(pageAliases([])).toEqual({})
  })
})

describe('when a width is given', () => {
  it.each([
    ['375', 375],
    ['240', 240],
    ['3840', 3840],
    [null, 'fill'],
    ['', 'fill'],
    ['239', 'fill'],
    ['3841', 'fill'],
    ['12.5', 'fill'],
    ['-400', 'fill'],
    ['wide', 'fill']
  ])('should read %j as %j', (value, expected) => {
    expect(parseWidth(value)).toBe(expected)
  })

  it('should offer one preset per breakpoint in order, the phone width for 0, then fill', () => {
    expect(widthPresets({ desktop: 1024, mobile: 0, tablet: 768 }).map((p) => p.width)).toEqual([
      360,
      768,
      1024,
      'fill'
    ])
  })

  it('should leave fill and empty values out of links', () => {
    expect(galleryHref({ component: 'Button', width: 'fill' })).toBe('?component=Button')
    expect(galleryHref({ width: undefined })).toBe('./')
  })
})
