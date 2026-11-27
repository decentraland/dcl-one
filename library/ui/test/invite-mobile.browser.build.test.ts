import { beforeAll, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/invite-mobile.tsx')

type Measured = {
  visible: boolean
  hit: boolean
  scrollY: number
  backgroundCoversHero: boolean
  avatarAboveTitle: boolean
  heroFitsScreen: boolean
}

// The first hero's background is its only aria-hidden box: AnimatedBackground's root.
const MEASURE = `(async()=>{
  for(let i=0;i<120&&![...document.links].some(l=>l.textContent.trim()==="JOIN NOW");i++)await new Promise(r=>requestAnimationFrame(r));
  await document.fonts.ready;
  await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
  const link=[...document.links].find(l=>l.textContent.trim()==="JOIN NOW"),{top,right,bottom,left}=link.getBoundingClientRect();
  const hero=document.querySelector("main section"),box=hero.getBoundingClientRect();
  const bg=hero.querySelector("[aria-hidden]").getBoundingClientRect();
  const avatar=hero.querySelector("[data-invite-avatar]").getBoundingClientRect();
  const title=hero.querySelector("h1").getBoundingClientRect();
  return{
    visible:top>=0&&left>=0&&bottom<=visualViewport.height&&right<=visualViewport.width,
    hit:document.elementFromPoint((left+right)/2,(top+bottom)/2)?.closest("a")===link,
    scrollY,
    backgroundCoversHero:bg.top<=box.top&&bg.bottom>=box.bottom&&bg.left<=box.left&&bg.right>=box.right,
    avatarAboveTitle:avatar.bottom<=title.top,
    heroFitsScreen:Math.round(box.height)<=innerHeight
  }
})()`

describe.each([
  { width: 485, height: 878 },
  { width: 390, height: 640 },
  { width: 320, height: 568 },
  { width: 320, height: 480 }
])('when the invite landing opens on a $width × $height phone', ({ width, height }) => {
  let measured: Measured

  beforeAll(async () => {
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: true
    })
    await browser.session.navigate(browser.origin)
    measured = await browser.evaluate<Measured>(MEASURE)
  })

  it('should expose the entire Join Now link to the invited player before scrolling', () => {
    expect(measured).toMatchObject({ visible: true, hit: true, scrollY: 0 })
  })

  it('should run the background behind the copy, with no edge across the first screen', () => {
    expect(measured.backgroundCoversHero).toBe(true)
  })

  it('should keep the avatar above the title', () => {
    expect(measured.avatarAboveTitle).toBe(true)
  })
})

describe.each([
  { width: 485, height: 878 },
  { width: 390, height: 640 },
  { width: 320, height: 480 }
])(
  'when the avatar drawn in the slot is taller than a $width × $height phone',
  ({ width, height }) => {
    let measured: Measured

    beforeAll(async () => {
      await browser.session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: true
      })
      await browser.session.navigate(`${browser.origin}?avatar=tall`)
      measured = await browser.evaluate<Measured>(MEASURE)
    })

    it('should keep the hero to one screen', () => {
      expect(measured.heroFitsScreen).toBe(true)
    })

    it('should still expose the entire Join Now link before scrolling', () => {
      expect(measured).toMatchObject({ visible: true, hit: true, scrollY: 0 })
    })
  }
)
