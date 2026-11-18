// @vitest-environment node
import { beforeEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/input-action-browser.tsx')
const evaluate = browser.evaluate
type Box = { x: number; y: number; width: number; height: number }
async function boxes(scope: string) {
  return evaluate<{ input: Box; action: Box | null; row: Box }>(`(()=>{
    const root=document.querySelector(${JSON.stringify(scope)})
    const box=(el)=>{if(!el)return null;const r=el.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}}
    const input=root.querySelector('input')
    const row=input.closest('[class*="fieldRow"]')
    return{input:box(input),action:box(root.querySelector('button')),row:box(row)}
  })()`)
}
async function at(width: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
  await evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
}
beforeEach(async () => {
  await browser.session.navigate(browser.origin)
  await evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("input");n++)await new Promise(r=>setTimeout(r,20))})()'
  )
})
describe('when a web field has an action', () => {
  it.each([390, 767])('should put the action under a full-width field at %ipx', async (width) => {
    await at(width)
    const { input, action, row } = await boxes('#web')
    expect(action!.y).toBeGreaterThanOrEqual(input.y + input.height)
    expect(action!.width).toBeCloseTo(row.width, 0)
    expect(input.width).toBeGreaterThan(row.width - 60)
    expect(browser.session.runtimeErrors).toHaveLength(0)
  })
  it('should keep the action beside the field from 768px', async () => {
    await at(768)
    const { input, action } = await boxes('#web')
    expect(action!.x).toBeGreaterThanOrEqual(input.x + input.width)
    expect(Math.abs(action!.y + action!.height / 2 - (input.y + input.height / 2))).toBeLessThan(20)
  })
})
describe('when the field is on the explorer surface', () => {
  it('should leave the explorer surface action beside its field on a phone', async () => {
    await at(390)
    const { input, action } = await boxes('#explorer')
    expect(action!.x).toBeGreaterThanOrEqual(input.x + input.width)
  })
})
