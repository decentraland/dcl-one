import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/invite-mobile.tsx')

describe.each([
  { width: 485, height: 878 },
  { width: 390, height: 640 },
  { width: 320, height: 568 },
  { width: 320, height: 480 }
])('when the invite landing opens on a $width × $height phone', ({ width, height }) => {
  it('should expose the entire Join Now link to the invited player before scrolling', async () => {
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: true
    })
    await browser.session.navigate(browser.origin)
    const measured = await browser.evaluate<{
      visible: boolean
      hit: boolean
      scrollY: number
    }>(
      `(async()=>{for(let i=0;i<120&&![...document.links].some(l=>l.textContent.trim()==="JOIN NOW");i++)await new Promise(r=>requestAnimationFrame(r));await document.fonts.ready;await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const link=[...document.links].find(l=>l.textContent.trim()==="JOIN NOW"),{top,right,bottom,left}=link.getBoundingClientRect();return{visible:top>=0&&left>=0&&bottom<=visualViewport.height&&right<=visualViewport.width,hit:document.elementFromPoint((left+right)/2,(top+bottom)/2)?.closest("a")===link,scrollY}})()`
    )
    expect(measured).toEqual({ visible: true, hit: true, scrollY: 0 })
  })
})
