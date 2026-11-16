import { expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
const browser = productionBrowser('test/consumer/jump-invalid-mobile.tsx')
it('should retain the fixed action inside the actual mobile visual viewport at every scroll position', async () => {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 1,
    mobile: true
  })
  await browser.session.send('Emulation.setTouchEmulationEnabled', {
    enabled: true,
    maxTouchPoints: 1
  })
  await browser.session.navigate(browser.origin)
  await browser.evaluate(
    `(async()=>{for(let i=0;i<120&&!document.querySelector('a[href="#home"]');i++)await new Promise(r=>requestAnimationFrame(r));await document.fonts.ready})()`
  )
  for (const y of [0, 200, 'footer']) {
    const measured = await browser.evaluate<{
      layoutWidth: number
      layoutHeight: number
      visualWidth: number
      visualHeight: number
      documentWidth: number
      sectionRight: number
      actionTop: number
      actionBottom: number
      hit: boolean
    }>(
      `(async()=>{scrollTo(0,${y === 'footer' ? 'document.documentElement.scrollHeight' : y});await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));const a=document.querySelector('a[href="#home"]'),r=a.getBoundingClientRect();return{layoutWidth:innerWidth,layoutHeight:innerHeight,visualWidth:visualViewport.width,visualHeight:visualViewport.height,documentWidth:document.documentElement.scrollWidth,sectionRight:document.querySelector('section').getBoundingClientRect().right,actionTop:r.top,actionBottom:r.bottom,hit:document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.closest('a')===a}})()`
    )
    expect(measured).toMatchObject({
      layoutWidth: 390,
      layoutHeight: 844,
      visualWidth: 390,
      visualHeight: 844,
      documentWidth: 390,
      hit: true
    })
    expect(measured.sectionRight).toBeLessThanOrEqual(390)
    expect(measured.actionTop).toBeGreaterThanOrEqual(0)
    expect(measured.actionBottom).toBeLessThanOrEqual(844)
  }
})
