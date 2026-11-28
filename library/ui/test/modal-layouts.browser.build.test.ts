// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/modal-layouts.tsx')

type Metrics = {
  x: number
  y: number
  width: number
  height: number
  radius: string
  padding: string
  position: string
  modal: boolean
  closeRight: number
  closeTop: number
  coverWidth: number
  pageScroll: string
}

async function open(query: string, width: number, height: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: width < 600
  })
  await browser.session.navigate(`${browser.origin}?${query}`)
  return browser.evaluate<Metrics>(`(async()=>{
    for(let i=0;i<120&&!document.querySelector('dialog[open]');i++)await new Promise(r=>requestAnimationFrame(r));
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    const d=document.querySelector('dialog'),r=d.getBoundingClientRect(),s=getComputedStyle(d);
    const c=d.querySelector('button[aria-label="Close"]').getBoundingClientRect();
    const cover=d.querySelector('[data-cover]');
    return {x:Math.round(r.x),y:Math.round(r.y),width:Math.round(r.width),height:Math.round(r.height),
      radius:s.borderTopLeftRadius,padding:s.paddingTop,position:s.position,modal:d.matches(':modal'),
      closeRight:Math.round(r.left+d.clientLeft+d.clientWidth-c.right),closeTop:Math.round(c.top-r.top),
      coverWidth:cover?Math.round(cover.getBoundingClientRect().width):0,
      pageScroll:document.documentElement.style.overflow}
  })()`)
}

afterEach(async () => {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width: 1440,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
})

describe('when the modal draws a message', () => {
  it('should keep its 560px card with 32px padding on a desktop', async () => {
    const m = await open('layout=message', 1440, 900)
    expect(m).toMatchObject({ width: 560, padding: '32px', radius: '16px', modal: true })
  })
})

describe('when the modal draws an event detail', () => {
  it('should draw an 880px card, its cover edge to edge, 90px clear of a tall detail', async () => {
    const m = await open('layout=detail&tall=1', 1440, 900)
    expect(m).toMatchObject({ x: 280, y: 90, width: 880, height: 720, radius: '16px' })
    expect(m).toMatchObject({ padding: '0px', closeRight: 16, closeTop: 16, modal: true })
    // A tall card scrolls inside: its scrollbar narrows the cover; the close button keeps 16px from it.
    expect(m.coverWidth).toBeGreaterThan(860)
  })

  it('should keep a short detail 90px from the top', async () => {
    const m = await open('layout=detail', 1440, 900)
    expect(m).toMatchObject({ y: 90, width: 880, modal: true })
    expect(m.height).toBeLessThan(720)
  })

  it('should be a full-height sheet below the site header on a phone', async () => {
    const m = await open('layout=detail&tall=1', 390, 844)
    expect(m).toMatchObject({ x: 0, y: 64, width: 390, height: 780, radius: '0px', modal: true })
  })
})

describe('when the modal draws a place card', () => {
  it('should follow the viewport from 680px on the scene card', async () => {
    const m = await open('layout=scene', 1440, 900)
    expect(m).toMatchObject({ width: 680, radius: '24px', padding: '0px', coverWidth: 680 })
    expect(m).toMatchObject({ closeRight: 16, closeTop: 16, modal: true })
  })

  it('should keep 24px clear of a phone edge', async () => {
    const m = await open('layout=scene', 390, 844)
    expect(m).toMatchObject({ x: 24, width: 342, modal: true })
  })
})

describe("when the place card is a page's own content", () => {
  it('should open modal above a phone and lock the page scroll', async () => {
    const m = await open('layout=page', 1440, 900)
    expect(m).toMatchObject({ width: 680, modal: true, pageScroll: 'hidden' })
  })

  it('should sit in the page flow on a phone, square and full width, the page still scrolling', async () => {
    const m = await open('layout=page', 390, 844)
    expect(m).toMatchObject({ width: 390, radius: '0px', position: 'relative', modal: false })
    expect(m).toMatchObject({ closeRight: 16, closeTop: 16, pageScroll: '' })
  })
})
