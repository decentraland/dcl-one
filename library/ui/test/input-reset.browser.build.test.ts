// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/input-reset-browser.tsx')
const evaluate = browser.evaluate
async function click(selector: string) {
  const p = await evaluate<{ x: number; y: number }>(
    `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`
  )
  for (const type of ['mousePressed', 'mouseReleased'])
    await browser.session.send('Input.dispatchMouseEvent', {
      type,
      button: 'left',
      clickCount: 1,
      ...p
    })
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
}
describe('when native password form reset reaches its final owner', () => {
  for (const surface of ['explorer'])
    for (const cancel of ['', 'form', 'document'])
      it(`should respect ${cancel || 'accepted'} ${surface} reset`, async () => {
        await browser.session.navigate(browser.origin + `?surface=${surface}&cancel=${cancel}`)
        await evaluate(
          '(async()=>{for(let n=0;n<100&&!document.querySelector("input");n++)await new Promise(r=>setTimeout(r,20))})()'
        )
        await click('button[aria-label="Show password"]')
        expect(await evaluate('document.querySelector("input").type')).toBe('text')
        await click('#reset')
        expect(await evaluate('document.body.dataset.canceled')).toBe(String(!!cancel))
        expect(await evaluate('document.querySelector("input").type')).toBe(
          cancel ? 'text' : 'password'
        )
      })
  for (const cancel of ['', 'document'])
    it(`should respect ${cancel || 'accepted'} reset for an external form`, async () => {
      await browser.session.navigate(browser.origin + `?external&cancel=${cancel}`)
      await evaluate(
        '(async()=>{for(let n=0;n<100&&!document.querySelector("input");n++)await new Promise(r=>setTimeout(r,20))})()'
      )
      await click('button[aria-label="Show password"]')
      await click('#reset')
      expect(await evaluate('document.querySelector("input").type')).toBe(
        cancel ? 'text' : 'password'
      )
    })
  it('should discard pending reset work when the form owner unmounts the input', async () => {
    await browser.session.navigate(browser.origin + '?unmount')
    await evaluate(
      '(async()=>{for(let n=0;n<100&&!document.querySelector("input");n++)await new Promise(r=>setTimeout(r,20))})()'
    )
    await click('button[aria-label="Show password"]')
    await click('#reset')
    expect(await evaluate('!!document.querySelector("input")')).toBe(false)
    expect(browser.session.runtimeErrors).toHaveLength(0)
  })
})

for (const control of ['#allow', '#field-type', '#disabled'])
  it(`should remask after a reveal eligibility transition through ${control}`, async () => {
    await browser.session.navigate(browser.origin)
    await evaluate(
      '(async()=>{for(let n=0;n<100&&!document.querySelector("input");n++)await new Promise(r=>setTimeout(r,20))})()'
    )
    await click('button[aria-label="Show password"]')
    expect(await evaluate('document.querySelector("input").type')).toBe('text')
    await click(control)
    if (control !== '#field-type')
      expect(await evaluate('document.querySelector("input").type')).toBe('password')
    await click(control)
    expect(await evaluate('document.querySelector("input").type')).toBe('password')
    expect(await evaluate('document.querySelector("input").value')).toBe('secret')
    expect(
      await evaluate('document.querySelector("button[aria-pressed]").getAttribute("aria-pressed")')
    ).toBe('false')
  })
