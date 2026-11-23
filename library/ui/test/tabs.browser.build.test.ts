// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/tabs-browser.tsx')
const evaluate = browser.evaluate
async function ready(width: number, label = 'Overview') {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: width < 768
  })
  await browser.session.send('Emulation.setTouchEmulationEnabled', {
    enabled: width < 768,
    maxTouchPoints: 1
  })
  await browser.session.navigate(browser.origin + '?label=' + encodeURIComponent(label))
  await evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("#tabs a");n++)await new Promise(r=>setTimeout(r,20));await document.fonts.ready})()'
  )
}
async function hoverFirst() {
  const point = await evaluate<{ x: number; y: number; outsideY: number }>(
    `(() => { const r=document.querySelector('#tabs a').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,outsideY:innerHeight-1} })()`
  )
  await browser.session.send('Input.dispatchMouseEvent', {
    type: 'mouseMoved',
    x: 0,
    y: point.outsideY,
    pointerType: 'mouse'
  })
  await browser.session.send('Input.dispatchMouseEvent', {
    type: 'mouseMoved',
    x: point.x,
    y: point.y,
    pointerType: 'mouse'
  })
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
  const observed = await evaluate<{
    hover: boolean
    fine: boolean
    matched: boolean
    hit: boolean
    font: string
    weight: string
  }>(`(() => {
    const e=document.querySelector('#tabs a'),r=e.getBoundingClientRect(),s=getComputedStyle(e);
    return {hover:matchMedia('(hover:hover)').matches,fine:matchMedia('(pointer:fine)').matches,
      matched:e.matches(':hover'),hit:document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.closest('a')===e,
      font:s.fontSize,weight:s.fontWeight,width:innerWidth,touch:navigator.maxTouchPoints,focused:document.hasFocus(),userAgent:navigator.userAgent};
  })()`)
  expect(observed, JSON.stringify(observed)).toMatchObject({
    hover: true,
    fine: true,
    matched: true,
    hit: true,
    font: '15px',
    weight: '700'
  })
}
describe('when public native route tabs render their captured content', () => {
  for (const width of [390, 1440])
    it(`should preserve native target and typography at${width}`, async () => {
      await ready(width)
      const actual = await evaluate<
        {
          height: number
          font: string
          weight: string
          current: string | null
          text: string
          name: string
          offset: string
        }[]
      >(
        `(()=>[...document.querySelectorAll('#tabs a')].map(e=>{const s=getComputedStyle(e),r=e.getBoundingClientRect();return{height:r.height,font:s.fontSize,weight:s.fontWeight,current:e.getAttribute('aria-current'),text:e.textContent,name:e.innerText,offset:getComputedStyle(e).getPropertyValue('--tab-underline-offset').trim()}}))()`
      )
      expect(actual[0]!.height).toBe(width < 768 ? 44 : 64)
      expect(actual[0]!.offset).toBe(width < 768 ? '9px' : '0px')
      expect(actual[1]!.height).toBe(width < 768 ? 44 : 64)
      expect(actual[0]!.font).toBe(width < 768 ? '12px' : '16px')
      expect(actual[1]!.font).toBe(width < 768 ? '12px' : '15px')
      expect(actual[1]!.current).toBe('page')
      expect(actual[0]!.text).toBe('Overview')
    })
  it('should retain adjacent link geometry across actual desktop hover', async () => {
    await ready(1440)
    const before = await evaluate<{ x: number; width: number }[]>(
      `(()=>[...document.querySelectorAll('#tabs a')].map(e=>{const r=e.getBoundingClientRect();return{x:r.x,width:r.width}}))()`
    )
    await hoverFirst()
    const after = await evaluate<{ x: number; width: number }[]>(
      `(()=>[...document.querySelectorAll('#tabs a')].map(e=>{const r=e.getBoundingClientRect();return{x:r.x,width:r.width}}))()`
    )
    expect(after).toEqual(before)
    expect(
      await evaluate(
        `(()=>{const e=document.querySelector('#tabs a'),s=getComputedStyle(e);return{font:s.fontSize,weight:s.fontWeight,label:e.querySelector('span').scrollWidth>e.querySelector('span').clientWidth}})()`
      )
    ).toEqual({ font: '15px', weight: '700', label: false })
  })
})
describe('when valid route captions use narrow and wide glyphs', () => {
  for (const label of ['III', 'IAP', 'I'.repeat(20), 'A'.repeat(20), 'WWW'])
    it(`should reserve both text styles for ${label}`, async () => {
      await ready(1440, label)
      const before = await evaluate<{ x: number; width: number }[]>(
        `(()=>[...document.querySelectorAll('#tabs a')].map(e=>{const r=e.getBoundingClientRect();return{x:r.x,width:r.width}}))()`
      )
      await hoverFirst()
      expect(
        await evaluate(
          `(()=>{const e=document.querySelector('#tabs a span');return e.scrollWidth>e.clientWidth})()`
        )
      ).toBe(false)
      expect(
        await evaluate(
          `(()=>[...document.querySelectorAll('#tabs a')].map(e=>{const r=e.getBoundingClientRect();return{x:r.x,width:r.width}}))()`
        )
      ).toEqual(before)
    })
})

type Bar = {
  height: number
  image: string
  size: string
  position: string
  shadow: string
  transform: string
  animations: string[]
}
const primaryBar = 'linear-gradient(rgb(255, 45, 85), rgb(255, 45, 85))'
const bars = (selector: string) =>
  evaluate<Bar[]>(
    `[...document.querySelectorAll(${JSON.stringify(selector)})].map(e=>{const s=getComputedStyle(e);return{height:e.getBoundingClientRect().height,image:s.backgroundImage.split('), linear')[0]+(s.backgroundImage.includes('), linear')?')':''),size:s.backgroundSize.split(',')[0],position:s.backgroundPosition.split(',')[0],shadow:s.boxShadow,transform:s.transform,animations:e.getAnimations().map(a=>a.transitionProperty??a.animationName)}})`
  )
const settled = () =>
  evaluate(
    '(async()=>{for(let n=0;n<120&&document.getAnimations().length;n++)await new Promise(r=>requestAnimationFrame(r))})()'
  )
async function click(selector: string) {
  const point = await evaluate<{ x: number; y: number }>(
    `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`
  )
  for (const type of ['mousePressed', 'mouseReleased'] as const)
    await browser.session.send('Input.dispatchMouseEvent', {
      type,
      ...point,
      button: 'left',
      clickCount: 1
    })
}
async function media(features: { name: string; value: string }[]) {
  await browser.session.send('Emulation.setEmulatedMedia', { features })
}

describe('when a tab is selected', () => {
  // The selected bar the tabs drew before it moved: 4px of primary along the tab's bottom edge (the route tabs lift it
  // 9px on phones), the height of each variant, and no bar on the other tab.
  for (const width of [390, 1440])
    it(`should end in the same underline as before the motion at ${width}`, async () => {
      await media([])
      await ready(width)
      await click('#underline [role=tab]:nth-child(2)')
      await click('#mobile [role=tab]:nth-child(2)')
      await click('#compact [role=tab]:nth-child(2)')
      await settled()
      for (const [selector, height] of [
        ['#underline [role=tab]', 64],
        ['#mobile [role=tab]', 35],
        ['#compact [role=tab]', width < 1024 ? 64 : 46]
      ] as const) {
        const [other, selected] = await bars(selector)
        expect(selected).toEqual({
          height,
          image: primaryBar,
          size: '100% 4px',
          position: '0px 100%',
          shadow: 'none',
          transform: 'none',
          animations: []
        })
        expect(other!.size).toBe('0% 4px')
      }
      const [route, current] = await bars('#tabs a')
      expect(current).toMatchObject({
        height: width < 768 ? 44 : 64,
        image: primaryBar,
        size: '100% 4px',
        position: width < 768 ? '0px calc(100% - 9px)' : '0px 100%'
      })
      expect(route!.size).toBe('0% 4px')
    })

  it('should grow the new bar and pop the new pill', async () => {
    await media([{ name: 'prefers-reduced-motion', value: 'no-preference' }])
    await ready(1440)
    await click('#underline [role=tab]:nth-child(2)')
    await click('#pill [role=tab]:nth-child(2)')
    const [, underline] = await bars('#underline [role=tab]')
    expect(underline!.animations).toContain('--tab-scale')
    expect(underline!.position).toBe('50% 100%')
    const [, pill] = await bars('#pill [role=tab]')
    expect(pill!.animations.some((name) => name.includes('pillPop'))).toBe(true)
  })

  // As the Explorer HUD's underline: the outgoing bar shrinks to its centre while the incoming one grows, both on
  // the 200ms spring, so 25ms in the old bar is about half its width (the HUD's scaleX reads 0.512 there).
  it('should shrink the outgoing bar to its centre while the new one grows', async () => {
    await media([{ name: 'prefers-reduced-motion', value: 'no-preference' }])
    for (const selector of ['#underline', '#compact']) {
      await ready(1440)
      await evaluate(
        `document.addEventListener('click',()=>setTimeout(()=>document.getAnimations().forEach(a=>a.pause())),{capture:true,once:true})`
      )
      await click(selector + ' [role=tab]:nth-child(2)')
      const at = (ms: number) =>
        evaluate<{ size: number; at: number }[]>(
          `(async()=>{
            document.getAnimations().forEach(a=>{a.pause();a.currentTime=${ms}})
            await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))
            return [...document.querySelectorAll(${JSON.stringify(selector + ' [role=tab]')})].slice(0,2).map(e=>{const s=getComputedStyle(e);return{size:parseFloat(s.backgroundSize),at:parseFloat(s.backgroundPositionX)}})
          })()`
        )
      // 25ms in, both bars are about half their width, centred.
      const [outgoing, incoming] = await at(25)
      expect(outgoing!.size).toBeGreaterThan(40)
      expect(outgoing!.size).toBeLessThan(60)
      expect(incoming!.size).toBeGreaterThan(40)
      expect(incoming!.size).toBeLessThan(60)
      expect([outgoing!.at, incoming!.at]).toEqual([50, 50])
      // 100ms in, the spring has carried the outgoing bar past zero: it is still drawn (the HUD's scaleX is -0.087),
      // and the incoming one is full width.
      const [late, grown] = await at(100)
      expect(late!.size).toBeGreaterThan(5)
      expect(late!.size).toBeLessThan(12)
      expect(grown!.size).toBeGreaterThanOrEqual(100)
      await evaluate('document.getAnimations().forEach(a=>a.finish())')
    }
  })

  it('should apply the selected state at once when motion is reduced', async () => {
    await media([{ name: 'prefers-reduced-motion', value: 'reduce' }])
    await ready(1440)
    await click('#underline [role=tab]:nth-child(2)')
    await click('#pill [role=tab]:nth-child(2)')
    const [, underline] = await bars('#underline [role=tab]')
    expect(underline).toMatchObject({ size: '100% 4px', position: '0px 100%', animations: [] })
    // Only the colour transitions the pill had before the motion remain.
    const [, pill] = await bars('#pill [role=tab]')
    expect(pill).toMatchObject({ transform: 'none', animations: ['background-color', 'color'] })
    // Hovering a pill does not scale it either.
    await browser.session.send('Input.dispatchMouseEvent', {
      type: 'mouseMoved',
      ...(await evaluate<{ x: number; y: number }>(
        `(()=>{const r=document.querySelector('#pill [role=tab]').getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`
      ))
    })
    await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
    expect((await bars('#pill [role=tab]'))[0]!.transform).toBe('none')
    expect(
      await evaluate('getComputedStyle(document.querySelector("#tabs a")).transitionDuration')
    ).toBe('0s')
    await media([])
  })

  it('should not scale a disabled pill on hover', async () => {
    await media([{ name: 'prefers-reduced-motion', value: 'no-preference' }])
    await ready(1440)
    const hover = async (selector: string) => {
      await browser.session.send('Input.dispatchMouseEvent', {
        type: 'mouseMoved',
        ...(await evaluate<{ x: number; y: number }>(
          `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`
        ))
      })
      await settled()
      return evaluate<string>(
        `getComputedStyle(document.querySelector(${JSON.stringify(selector)})).transform`
      )
    }
    expect(await hover('#pill [role=tab]:nth-child(2)')).not.toBe('none')
    expect(await hover('#pill [role=tab]:disabled')).toBe('none')
  })

  describe('and colours are forced', () => {
    type Forced = {
      system: Record<'Highlight' | 'GrayText' | 'LinkText', string>
      panels: {
        width: number
        bar: { width: number; height: string; color: string; bottom: string }
      }[]
      disabled: string[]
      idle: string
    }
    const read = () =>
      evaluate<Forced>(`(()=>{
        const probe=document.createElement('span');probe.style.forcedColorAdjust='none';document.body.append(probe)
        const system=Object.fromEntries(['Highlight','GrayText','LinkText'].map(n=>{probe.style.color=n;return[n,getComputedStyle(probe).color]}))
        probe.remove()
        const panels=['underline','mobile','compact'].map(id=>{
          const tab=document.querySelector('#'+id+' [aria-selected=true]'),bar=getComputedStyle(tab,'::after')
          return{width:tab.getBoundingClientRect().width,bar:{width:parseFloat(bar.width),height:bar.height,color:bar.backgroundColor,bottom:bar.bottom}}
        })
        const disabled=['underline','pill'].map(id=>getComputedStyle(document.querySelector('#'+id+' [role=tab]:disabled')).color)
        return{system,panels,disabled,idle:getComputedStyle(document.querySelector('#tabs a:not([aria-current])')).color}
      })()`)

    it('should draw the selection in Highlight and keep the system colours of the tabs', async () => {
      await media([{ name: 'forced-colors', value: 'active' }])
      await ready(1440)
      const { system, panels, disabled, idle } = await read()
      expect(system.Highlight).not.toBe(system.GrayText)
      for (const { width, bar } of panels) {
        expect(bar.width).toBeCloseTo(width, 3)
        expect(bar).toMatchObject({ height: '4px', color: system.Highlight, bottom: '0px' })
      }
      expect(disabled).toEqual([system.GrayText, system.GrayText])
      expect(idle).toBe(system.LinkText)
      await media([])
    })

    // The colours a band of the page paints, read from a screenshot of it.
    async function paint(band: { x: number; y: number; width: number; height: number }) {
      const shot = await browser.session.send('Page.captureScreenshot', {
        format: 'png',
        clip: { ...band, scale: 1 }
      })
      const data = (shot as { data: string }).data
      return evaluate<string[]>(`(async()=>{
        const img=new Image();img.src='data:image/png;base64,${data}';await img.decode()
        const c=document.createElement('canvas');c.width=img.width;c.height=img.height
        const g=c.getContext('2d');g.drawImage(img,0,0);const d=g.getImageData(0,0,c.width,c.height).data
        const seen=new Set();for(let i=0;i<d.length;i+=4)seen.add(d[i]+','+d[i+1]+','+d[i+2]);return[...seen].sort()
      })()`)
    }
    type Route = {
      link: { x: number; y: number; width: number; height: number }
      label: { x: number; y: number; width: number; height: number }
      offset: number
    }
    const routes = () =>
      evaluate<Route[]>(
        `[...document.querySelectorAll('#tabs a')].map(a=>{const box=r=>({x:r.x,y:r.y,width:r.width,height:r.height});return{link:box(a.getBoundingClientRect()),label:box(a.querySelector('span').getBoundingClientRect()),offset:parseFloat(getComputedStyle(a).getPropertyValue('--tab-underline-offset'))}})`
      )

    for (const width of [390, 1440])
      it(`should mark only the current route, without moving any label, at ${width}`, async () => {
        await media([])
        await ready(width)
        const base = await routes()
        await media([{ name: 'forced-colors', value: 'active' }])
        await ready(width)
        const forced = await routes()
        expect(forced).toEqual(base)
        const [idle, current] = forced
        const band = ({ link, offset }: Route, top = false) => ({
          x: link.x + 1,
          y: top ? link.y : link.y + link.height - offset - 4,
          width: link.width - 2,
          height: 4
        })
        // The idle route paints its bar band exactly as its empty top edge: no bar.
        const empty = await paint(band(idle!, true))
        expect(empty).toHaveLength(1)
        expect(await paint(band(idle!))).toEqual(empty)
        // The current route paints one colour across the band, and it is not the background.
        const marker = await paint(band(current!))
        expect(marker).toHaveLength(1)
        expect(marker).not.toEqual(empty)
        await media([])
      })
  })
})
