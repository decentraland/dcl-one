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
    await browser.session.send('Input.dispatchMouseEvent', {
      type: 'mouseMoved',
      x: before[0]!.x + before[0]!.width / 2,
      y: 40
    })
    await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
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
      await browser.session.send('Input.dispatchMouseEvent', {
        type: 'mouseMoved',
        x: before[0]!.x + before[0]!.width / 2,
        y: 40
      })
      await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
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
