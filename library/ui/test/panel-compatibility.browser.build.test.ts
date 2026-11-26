import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/panel-compatibility-browser.tsx')
describe('when legacy panels render from the production bundle', () => {
  for (const width of [390, 1440])
    describe(`and the viewport is ${width}px wide`, () => {
      it('should preserve actual legacy panel geometry and trusted keyboard', async () => {
        await browser.session.send('Emulation.setDeviceMetricsOverride', {
          width,
          height: 844,
          deviceScaleFactor: 1,
          mobile: width < 768
        })
        await browser.session.send('Emulation.setTouchEmulationEnabled', {
          enabled: width < 768,
          maxTouchPoints: 1
        })
        await browser.session.navigate(browser.origin)
        await browser.evaluate(
          '(async()=>{for(let i=0;i<120&&!document.querySelector("[role=tab]");i++)await new Promise(r=>requestAnimationFrame(r));await document.fonts.ready})()'
        )
        const before = await browser.evaluate<{
          height: number
          role: string | null
          selected: string | null
          route: boolean
          anchorRole: string | null
        }>(
          `(()=>{const row=document.querySelector('[aria-label="Legacy panels"]'),a=row.querySelectorAll('[role=tab]');a[0].focus();return{height:a[0].getBoundingClientRect().height,role:row.getAttribute('role'),selected:a[0].getAttribute('aria-selected'),route:row.querySelector('[aria-current]')!==null,anchorRole:document.querySelector('a[href="#panel"]').getAttribute('role')}})()`
        )
        expect(before).toEqual({
          height: width < 768 ? 35 : 64,
          role: 'tablist',
          selected: 'true',
          route: false,
          anchorRole: 'tab'
        })
        await browser.session.send('Input.dispatchKeyEvent', {
          type: 'keyDown',
          key: 'ArrowRight',
          code: 'ArrowRight',
          windowsVirtualKeyCode: 39
        })
        await browser.session.send('Input.dispatchKeyEvent', {
          type: 'keyUp',
          key: 'ArrowRight',
          code: 'ArrowRight',
          windowsVirtualKeyCode: 39
        })
        await browser.evaluate(
          'new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))'
        )
        expect(
          await browser.evaluate(
            `(()=>{const a=document.querySelector('[aria-label="Legacy panels"]').querySelectorAll('[role=tab]');return{focused:document.activeElement===a[1],selected:a[1].getAttribute('aria-selected')}})()`
          )
        ).toEqual({ focused: true, selected: 'true' })
      })
    })
})
