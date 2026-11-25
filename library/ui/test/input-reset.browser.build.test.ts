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
      describe(`and the ${surface} reset is ${cancel ? `canceled by the ${cancel}` : 'accepted'}`, () => {
        it(`should ${cancel ? 'keep' : 'remask'} the revealed password and record the cancellation state`, async () => {
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
      })
  for (const cancel of ['', 'document'])
    describe(`and an external form reset is ${cancel ? `canceled by the ${cancel}` : 'accepted'}`, () => {
      it(`should ${cancel ? 'keep the revealed' : 'remask the'} password`, async () => {
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
    })
  describe('and the form owner unmounts the input', () => {
    it('should discard pending reset work without runtime errors', async () => {
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
})

describe('when a revealed password goes through a reveal eligibility transition', () => {
  for (const control of ['#allow', '#field-type', '#disabled'])
    describe(`and the transition is driven by ${control}`, () => {
      it('should remask the password and keep its value', async () => {
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
          await evaluate(
            'document.querySelector("button[aria-label]")?.hasAttribute("aria-pressed")'
          )
        ).toBe(false)
      })
    })
})
