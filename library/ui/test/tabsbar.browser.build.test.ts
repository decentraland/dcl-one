// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/tabsbar-browser.tsx')
async function navigate(page: string, width: number, activeId?: string) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
  await browser.session.navigate(
    browser.origin + '?page=' + page + (activeId ? '&active=' + activeId : '')
  )
  await browser.evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("nav");n++)await new Promise(r=>setTimeout(r,20));await document.fonts.ready})()'
  )
}
for (const page of ['create', 'shop', 'learn'])
  describe('when ' + page + ' uses its responsive composition', () => {
    for (const width of [390, 1024, 1920])
      it('should preserve parent geometry and single control instances at ' + width, async () => {
        await navigate(page, width)
        const value = await browser.evaluate<{
          height: number
          width: number
          gap: string
          navigationY: number
          controlsY: number | null
          controlsHeight: number | null
          searches: number
          links: number
          overflow: boolean
        }>(
          `(()=>{const root=document.querySelector('nav'),list=root.querySelector('ul'),controls=root.querySelector('[data-region=controls]');return{height:root.getBoundingClientRect().height,width:root.getBoundingClientRect().width,gap:getComputedStyle(list).columnGap,navigationY:list.getBoundingClientRect().y,controlsY:controls?.getBoundingClientRect().y??null,controlsHeight:controls?.getBoundingClientRect().height??null,searches:document.querySelectorAll('input').length,links:[...root.querySelectorAll('a')].filter(a=>a.getBoundingClientRect().width>0).length,overflow:document.documentElement.scrollWidth>innerWidth}})()`
        )
        expect(value.width).toBe(width)
        expect(value.height).toBe(
          width === 390 ? (page === 'create' ? 44 : page === 'shop' ? 120 : 124) : 64
        )
        expect(value.gap).toBe(width === 390 ? '16px' : width === 1920 ? '50px' : '32px')
        expect(value.searches).toBe(page === 'create' ? 0 : 1)
        expect(value.links).toBe(
          page === 'create' ? 5 : page === 'learn' ? 4 : width === 390 ? 4 : 5
        )
        expect(value.overflow).toBe(false)
        if (page !== 'create')
          expect(value.controlsHeight).toBe(width === 390 && page === 'learn' ? 68 : 64)
        if (width === 390 && page !== 'create')
          expect(value.controlsY!).toBeGreaterThan(value.navigationY)
      })
  })
describe('when the viewport crosses a shared foundation breakpoint', () => {
  for (const [width, height, gap] of [
    [767, 44, '16px'],
    [768, 64, '32px'],
    [1535, 64, '32px'],
    [1536, 64, '50px']
  ] as const)
    it('should preserve the create layout at ' + width, async () => {
      await navigate('create', width)
      expect(
        await browser.evaluate('document.querySelector("nav").getBoundingClientRect().height')
      ).toBe(height)
      expect(
        await browser.evaluate('getComputedStyle(document.querySelector("ul")).columnGap')
      ).toBe(gap)
    })
  it('should retain the search node, current route and typed value across responsive changes', async () => {
    await navigate('shop', 1920)
    await browser.evaluate(
      'window.fixtureInput=document.querySelector("input");window.fixtureInput.focus()'
    )
    await browser.session.send('Input.insertText', { text: 'Draft' })
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width: 390,
      height: 844,
      deviceScaleFactor: 1,
      mobile: true
    })
    expect(await browser.evaluate('window.fixtureInput===document.querySelector("input")')).toBe(
      true
    )
    expect(await browser.evaluate('document.querySelector("input").value')).toBe('Draft')
    expect(
      await browser.evaluate('document.querySelector("[aria-current]").getAttribute("href")')
    ).toBe('#0')
    await browser.evaluate('document.querySelectorAll("nav a")[2].focus()')
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Enter',
      code: 'Enter',
      windowsVirtualKeyCode: 13
    })
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Enter',
      code: 'Enter',
      windowsVirtualKeyCode: 13
    })
    expect(await browser.evaluate('document.querySelector("input").value')).toBe('Draft')
    expect(
      await browser.evaluate('document.querySelector("[aria-current]").getAttribute("href")')
    ).toBe('#2')
  })
  it('should keep a long focused route visible inside horizontal scrolling without page overflow', async () => {
    await navigate('learn', 390)
    await browser.session.send('Runtime.evaluate', {
      expression: 'document.getElementById("outside").focus()'
    })
    await tab(true)
    await tab(true)
    await tab(true)
    const focused = await browser.evaluate<{
      active: boolean
      visible: boolean
      outline: string
      scrollbar: string
    }>(
      `(()=>{const a=[...document.querySelectorAll('nav a')].at(-1);const r=a.getBoundingClientRect(),p=a.closest('ul').getBoundingClientRect();return{active:document.activeElement===a,visible:r.right<=p.right+1&&r.left>=p.left-1,outline:getComputedStyle(a).outlineStyle,scrollbar:getComputedStyle(a.closest('ul'),'::-webkit-scrollbar').display}})()`
    )
    expect(focused.active).toBe(true)
    expect(focused.scrollbar).toBe('none')
    expect(focused.visible).toBe(true)
    expect(focused.outline).toBe('solid')
  })
})
describe('when desktop controls compete with a narrow navigation row', () => {
  it('should leave the first shop route fully reachable at the tablet boundary', async () => {
    await navigate('shop', 768)
    expect(
      await browser.evaluate(
        `(()=>{const nav=document.querySelector('[data-region=navigation]').getBoundingClientRect(),link=document.querySelector('nav a').getBoundingClientRect();return nav.width>=link.width})()`
      )
    ).toBe(true)
  })
})
async function tab(reverse = false) {
  await browser.session.send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'Tab',
    code: 'Tab',
    windowsVirtualKeyCode: 9,
    modifiers: reverse ? 8 : 0
  })
  await browser.session.send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'Tab',
    code: 'Tab',
    windowsVirtualKeyCode: 9,
    modifiers: 0
  })
}
async function focusName() {
  return browser.evaluate<string>(
    `(()=>{const e=document.activeElement;return e.id==='outside'?'outside':e.tagName==='A'?e.getAttribute('href'):e.getAttribute('aria-label')??e.textContent.trim()})()`
  )
}
for (const page of ['create', 'shop', 'learn'])
  describe('when traversing the complete ' + page + ' composition', () => {
    for (const width of [390, 1024, 1920])
      it('should preserve one logical forward and reverse keyboard path at ' + width, async () => {
        await navigate(page, width)
        const routes =
          page === 'create'
            ? ['#0', '#1', '#2', '#3', '#4']
            : page === 'learn'
              ? ['#0', '#1', '#2', '#3']
              : width === 390
                ? ['#0', '#2', '#3', '#4']
                : ['#0', '#1', '#2', '#3', '#4']
        const controls =
          page === 'create'
            ? []
            : page === 'learn'
              ? ['Fixture search', 'Sort by']
              : [
                  'Fixture search',
                  ...(width === 390 ? [] : ['Buy credits']),
                  'Fixture favorites',
                  'Fixture activity',
                  'Fixture cart, 0'
                ]
        const path = [...routes, ...controls, 'outside']
        const forward: string[] = []
        for (let n = 0; n < path.length; n++) {
          await tab()
          forward.push(await focusName())
        }
        expect(forward).toEqual(path)
        const backward: string[] = []
        for (let n = 0; n < path.length - 1; n++) {
          await tab(true)
          backward.push(await focusName())
        }
        expect(backward).toEqual(path.slice(0, -1).reverse())
      })
  })

describe('when the current route is normally hidden on mobile', () => {
  it('should keep the active route visible, named and reachable by native Tab', async () => {
    await navigate('shop', 390, '1')
    expect(
      await browser.evaluate(
        `(()=>{const a=document.querySelector('[aria-current="page"]'),r=a.getBoundingClientRect();return r.width>0&&r.height>=44&&a.getAttribute('href')==='#1'})()`
      )
    ).toBe(true)
    await tab()
    await tab()
    expect(await focusName()).toBe('#1')
  })
})
