// @vitest-environment node
import { describe, it, expect } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/titlebar-browser.tsx')
async function navigate(variant: string, width = 1920, closed = false) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
  await browser.session.navigate(
    browser.origin + '?variant=' + variant + (closed ? '&closed=1' : '')
  )
  await browser.evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("[data-variant]");n++)await new Promise(r=>setTimeout(r,20));await document.fonts.ready})()'
  )
}
for (const variant of ['backpack', 'settings', 'events', 'places', 'shop'])
  describe('when rendering the native ' + variant + ' toolbar', () => {
    it('should match the desktop parent frame, title metrics, selected paint and control order', async () => {
      await navigate(variant)
      const result = await browser.evaluate<{
        height: number
        width: number
        titleSize: string
        titleWeight: string
        selected: string
        controls: string[]
        overflow: boolean
      }>(
        `(()=>{const r=document.querySelector('[data-variant]'),t=r.querySelector('h2'),s=r.querySelector('[data-active]');return{height:r.getBoundingClientRect().height,width:r.getBoundingClientRect().width,titleSize:getComputedStyle(t).fontSize,titleWeight:getComputedStyle(t).fontWeight,selected:getComputedStyle(s).backgroundColor,controls:[...r.querySelectorAll('[data-region=controls]>[data-control]')].map(e=>e.dataset.control),overflow:document.documentElement.scrollWidth>innerWidth}})()`
      )
      expect(result.height).toBe(68)
      expect(result.width).toBe(1920)
      expect(result.titleSize).toBe('36px')
      expect(result.titleWeight).toBe('600')
      expect(result.selected).toBe('rgb(255, 116, 57)')
      expect(result.controls).toEqual(
        variant === 'settings'
          ? []
          : variant === 'backpack'
            ? ['collection', 'filters', 'search']
            : variant === 'events'
              ? ['date', 'create']
              : variant === 'places'
                ? ['filters', 'search']
                : ['filters', 'sort', 'search', 'cart']
      )
      expect(result.overflow).toBe(false)
    })
    it('should wrap at narrow widths without duplicating or losing the supplied controls', async () => {
      await navigate(variant, 390)
      expect(await browser.evaluate('document.documentElement.scrollWidth>innerWidth')).toBe(false)
      expect(
        await browser.evaluate('document.querySelectorAll("[data-region=primary]").length')
      ).toBe(1)
      expect(
        await browser.evaluate('document.querySelectorAll("[data-control=search] input").length')
      ).toBe(['backpack', 'places', 'shop'].includes(variant) ? 1 : 0)
    })
  })
describe('when a caller supplies a Places filter panel', () => {
  it('should place the desktop panel outside the68px frame without clipping', async () => {
    await navigate('places')
    const p = await browser.evaluate<{
      top: number
      bottom: number
      height: number
      width: number
      overflow: string
    }>(
      `(()=>{const r=document.querySelector('[data-variant]'),m=r.querySelector('[data-control=filterMenu]');return{top:m.getBoundingClientRect().top,bottom:r.getBoundingClientRect().bottom,height:m.getBoundingClientRect().height,width:m.getBoundingClientRect().width,overflow:getComputedStyle(r).overflow}})()`
    )
    expect(p.top).toBe(p.bottom)
    expect(p.height).toBeGreaterThan(68)
    expect(p.width).toBe(240)
    expect(p.overflow).toBe('visible')
  })
  it('should keep the panel entirely in view after its trigger wraps to the left edge', async () => {
    await navigate('places', 390)
    expect(
      await browser.evaluate(
        `(()=>{const r=document.querySelector('[data-control=filterMenu]').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth})()`
      )
    ).toBe(true)
  })
})
describe('when a section is selected', () => {
  it('should keep its source-orange background and a readable text contrast', async () => {
    await navigate('settings')
    const ratio = await browser.evaluate<number>(
      `(()=>{const s=getComputedStyle(document.querySelector('[data-active]'));const L=c=>{const v=c.match(/[0-9.]+/g).slice(0,3).map(n=>{const x=Number(n)/255;return x<=.04045?x/12.92:((x+.055)/1.055)**2.4});return .2126*v[0]+.7152*v[1]+.0722*v[2]};const a=L(s.color),b=L(s.backgroundColor);return(Math.max(a,b)+.05)/(Math.min(a,b)+.05)})()`
    )
    expect(ratio).toBeGreaterThanOrEqual(4.5)
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
    `(()=>{const e=document.activeElement;return e.id==='outside'?'outside':e.getAttribute('aria-label')??e.textContent.trim()})()`
  )
}
const sectionNames = {
  backpack: ['Wearables', 'Emotes'],
  settings: ['Graphics', 'Audio', 'Controls', 'Chat', 'Lobby'],
  events: ['Calendar', 'Remind Me', 'Host'],
  places: ['Explore', 'Recent', 'Favorites', 'My Places'],
  shop: ['All', 'Wearables', 'Emotes', 'Names']
}
for (const variant of ['backpack', 'settings', 'events', 'places', 'shop'] as const)
  describe('when traversing the full ' + variant + ' toolbar', () => {
    for (const width of [390, 1920])
      it(
        'should preserve meaningful forward and reverse native focus order at ' + width,
        async () => {
          await navigate(variant, width, true)
          const controls =
            variant === 'settings'
              ? []
              : variant === 'backpack'
                ? ['Fixture collection', 'Filter & Sort', 'Fixture search']
                : variant === 'events'
                  ? ['Today', 'create hangout']
                  : variant === 'places'
                    ? ['Filter & Sort', 'Fixture search']
                    : ['Filters', 'Fixture shop sort', 'Fixture search', 'Fixture cart, 0']
          const order = [...sectionNames[variant], ...controls, 'outside']
          const forward: string[] = []
          for (let n = 0; n < order.length; n++) {
            await tab()
            forward.push(await focusName())
          }
          expect(forward).toEqual(order)
          const reverse: string[] = []
          for (let n = 0; n < order.length - 1; n++) {
            await tab(true)
            reverse.push(await focusName())
          }
          expect(reverse).toEqual(order.slice(0, -1).reverse())
        }
      )
  })
describe('when the app controls section state and panel lifetime', () => {
  it('should use native keyboard activation and preserve a typed search across selection and resize', async () => {
    await navigate('backpack')
    await browser.evaluate(
      'window.fixtureSearch=document.querySelector("[data-control=search] input");window.fixtureSearch.focus()'
    )
    await browser.session.send('Input.insertText', { text: 'Draft' })
    await browser.evaluate(
      'document.querySelector("[data-region=primary] button:not([aria-current])").focus()'
    )
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      text: '\r',
      unmodifiedText: '\r',
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
    expect(
      await browser.evaluate('document.querySelector("[aria-current=true]").textContent.trim()')
    ).toBe('Wearables')
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width: 390,
      height: 844,
      deviceScaleFactor: 1,
      mobile: true
    })
    expect(
      await browser.evaluate(
        'window.fixtureSearch===document.querySelector("[data-control=search] input")'
      )
    ).toBe(true)
    expect(
      await browser.evaluate('document.querySelector("[data-control=search] input").value')
    ).toBe('Draft')
  })
  it('should let the supplied popup own Escape and return focus to its still-mounted trigger', async () => {
    await navigate('places')
    await browser.evaluate('document.querySelector("[data-control=filterMenu] input").focus()')
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27
    })
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27
    })
    expect(
      await browser.evaluate('document.querySelector("[data-control=filterMenu]")===null')
    ).toBe(true)
    expect(await browser.evaluate('document.activeElement.textContent.trim()')).toBe(
      'Filter & Sort'
    )
    expect(await browser.evaluate('document.activeElement.getAttribute("aria-expanded")')).toBe(
      'false'
    )
  })
})
describe('when the selected section receives keyboard focus', () => {
  it('should keep a bright visible ring independent of the darker selected text', async () => {
    await navigate('settings')
    await tab()
    const focus = await browser.evaluate<{
      selected: boolean
      style: string
      ring: string
      text: string
    }>(
      `(()=>{const e=document.activeElement,s=getComputedStyle(e);return{selected:e.getAttribute('aria-current')==='true',style:s.outlineStyle,ring:s.outlineColor,text:s.color}})()`
    )
    expect(focus.selected).toBe(true)
    expect(focus.style).toBe('solid')
    expect(focus.ring).toBe('rgb(252, 252, 252)')
    expect(focus.text).toBe('rgb(22, 21, 24)')
  })
})
describe('when pointer focus enters the supplied filter panel padding', () => {
  it('should keep Escape owned by the panel and return to its trigger', async () => {
    await navigate('places')
    const point = await browser.evaluate<{ x: number; y: number }>(
      `(()=>{const r=document.getElementById('fixture-filters').getBoundingClientRect();return{x:r.x+3,y:r.y+3}})()`
    )
    for (const type of ['mousePressed', 'mouseReleased'])
      await browser.session.send('Input.dispatchMouseEvent', {
        type,
        button: 'left',
        clickCount: 1,
        ...point
      })
    expect(await browser.evaluate('document.activeElement.id')).toBe('fixture-filters')
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27
    })
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27
    })
    expect(
      await browser.evaluate('document.querySelector("[data-control=filterMenu]")===null')
    ).toBe(true)
    expect(await browser.evaluate('document.activeElement.textContent.trim()')).toBe(
      'Filter & Sort'
    )
  })
})

describe('when forced colors replace selected text and backgrounds', () => {
  for (const width of [390, 1920])
    it(
      'should preserve distinct selected and focus cues without geometry changes at ' + width,
      async () => {
        await navigate('settings', width, true)
        const before = await browser.evaluate<{
          x: number
          y: number
          width: number
          height: number
        }>(
          `(()=>{const r=document.querySelector('[data-active]').getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}})()`
        )
        await browser.session.send('Emulation.setEmulatedMedia', {
          features: [{ name: 'forced-colors', value: 'active' }]
        })
        try {
          await tab()
          const after = await browser.evaluate<{
            rect: { x: number; y: number; width: number; height: number }
            forced: boolean
            focused: boolean
            border: string
            borderWidth: string
            pointer: string
            ring: string
            ringWidth: string
            offset: string
          }>(
            `(()=>{const e=document.querySelector('[data-active]'),r=e.getBoundingClientRect(),s=getComputedStyle(e),p=getComputedStyle(e,'::after');return{rect:{x:r.x,y:r.y,width:r.width,height:r.height},forced:matchMedia('(forced-colors:active)').matches,focused:document.activeElement===e,border:p.borderTopStyle,borderWidth:p.borderTopWidth,pointer:p.pointerEvents,ring:s.outlineStyle,ringWidth:s.outlineWidth,offset:s.outlineOffset}})()`
          )
          expect(after.forced).toBe(true)
          expect(after.focused).toBe(true)
          expect(after.rect).toEqual(before)
          expect(after.border).toBe('solid')
          expect(after.borderWidth).toBe('2px')
          expect(after.pointer).toBe('none')
          expect(after.ring).toBe('solid')
          expect(after.ringWidth).toBe('2px')
          expect(after.offset).toBe('2px')
        } finally {
          await browser.session.send('Emulation.setEmulatedMedia', { features: [] })
        }
      }
    )
})
