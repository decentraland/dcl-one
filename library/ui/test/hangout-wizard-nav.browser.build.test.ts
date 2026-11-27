import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/hangout-wizard-nav-browser.tsx')

type Measured = {
  documentWidth: number
  clientWidth: number
  steps: {
    step: string
    cardLeft: number
    cardRight: number
    rows: number
    actions: { label: string; left: number; right: number }[]
  }[]
}

async function measure(width: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: width < 768
  })
  await browser.session.navigate(browser.origin)
  return browser.evaluate<Measured>(
    `(async()=>{for(let i=0;i<120&&!document.querySelector('[data-step=preview] button');i++)await new Promise(r=>requestAnimationFrame(r));await document.fonts.ready;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));return{documentWidth:document.documentElement.scrollWidth,clientWidth:document.documentElement.clientWidth,steps:[...document.querySelectorAll('[data-step]')].map(s=>{const card=s.querySelector('.submit-hangout__controls').getBoundingClientRect();const actions=[...s.querySelectorAll('.submit-hangout__nav > *')].map(a=>{const r=a.getBoundingClientRect();return{label:a.textContent.trim(),left:r.left,right:r.right,top:r.top}});return{step:s.dataset.step,cardLeft:card.left,cardRight:card.right,rows:new Set(actions.map(a=>Math.round(a.top))).size,actions:actions.map(({label,left,right})=>({label,left,right}))}})}})()`
  )
}

describe('when the hangout wizard draws its step actions', () => {
  it.each([390, 1440])('should keep every action inside the card at %ipx', async (width) => {
    const measured = await measure(width)
    // No horizontal scroll: a classic scrollbar narrows the layout, so the page compares with its own width.
    expect(measured.documentWidth).toBeLessThanOrEqual(measured.clientWidth)
    expect(measured.clientWidth).toBeGreaterThan(width - 20)
    expect(measured.steps.map((step) => step.step)).toEqual(['cover', 'review', 'preview'])
    for (const step of measured.steps) {
      for (const action of step.actions) {
        expect(action.left, `${step.step} ${action.label}`).toBeGreaterThanOrEqual(step.cardLeft)
        expect(action.right, `${step.step} ${action.label}`).toBeLessThanOrEqual(step.cardRight)
      }
    }
  })

  it('should wrap the review step onto a second row on a phone and keep one row on a desktop', async () => {
    expect((await measure(390)).steps.find((step) => step.step === 'review')?.rows).toBe(2)
    expect((await measure(1440)).steps.find((step) => step.step === 'review')?.rows).toBe(1)
  })
})
