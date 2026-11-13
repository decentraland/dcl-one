// @vitest-environment node
import { describe, it, expect } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/component-composition-browser.tsx')
const evaluate = browser.evaluate
async function ready() {
  await browser.session.navigate(browser.origin)
  await evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("#outside");n++)await new Promise(r=>setTimeout(r,20));await document.fonts.ready})()'
  )
}
async function clickOutside() {
  const point = await evaluate<{ x: number; y: number }>(
    '(()=>{const r=document.querySelector("#outside").getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()'
  )
  for (const type of ['mousePressed', 'mouseReleased'])
    await browser.session.send('Input.dispatchMouseEvent', {
      type,
      button: 'left',
      clickCount: 1,
      ...point
    })
}
async function previousFocus() {
  await browser.session.send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'Tab',
    code: 'Tab',
    windowsVirtualKeyCode: 9,
    modifiers: 8
  })
  await browser.session.send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'Tab',
    code: 'Tab',
    windowsVirtualKeyCode: 9,
    modifiers: 0
  })
}
describe('when public components share a production CSS bundle', () => {
  it('should preserve native tab typography when Button CSS is emitted first', async () => {
    await ready()
    const values = await evaluate<{ size: string; weight: string; clipped: boolean }[]>(
      '(()=>[...document.querySelectorAll("#mixed-tabs a")].map(e=>({size:getComputedStyle(e).fontSize,weight:getComputedStyle(e).fontWeight,clipped:e.querySelector("span").scrollWidth>e.querySelector("span").clientWidth})))()'
    )
    expect(values).toEqual([
      { size: '15px', weight: '700', clipped: false },
      { size: '16px', weight: '600', clipped: false }
    ])
  })
  for (const [id, steps] of [
    ['web-invalid', 1],
    ['explorer-invalid', 2]
  ] as const)
    it(`should preserve a distinct keyboard focus cue on ${id}`, async () => {
      await ready()
      await clickOutside()
      const shadow = () =>
        evaluate<string>(
          `getComputedStyle(document.getElementById('${id}').parentElement).boxShadow`
        )
      const before = await shadow()
      for (let n = 0; n < steps; n++) await previousFocus()
      expect(await evaluate('document.activeElement.id')).toBe(id)
      const after = await shadow()
      expect(after).not.toBe(before)
      expect(after).toContain(before)
    })
})
