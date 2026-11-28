// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/modal-drawer.tsx')

type Metrics = {
  x: number
  y: number
  width: number
  height: number
  padding: string
  radius: string
  background: string
  backdrop: string
  modal: boolean
  titleSize: string
  titleTextTop: number
  closeX: number
  closeY: number
  closeSize: number
  sectionY: number
}

async function open(width: number, height: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: width < 600
  })
  await browser.session.navigate(browser.origin)
  return browser.evaluate<Metrics>(`(async()=>{
    for(let i=0;i<120&&!document.querySelector('dialog[open]');i++)await new Promise(r=>requestAnimationFrame(r));
    await document.fonts.ready;
    await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
    const d=document.querySelector('dialog'),r=d.getBoundingClientRect(),s=getComputedStyle(d);
    const h=d.querySelector('h2'),range=document.createRange();range.selectNodeContents(h);
    const c=d.querySelector('button[aria-label="Close filters"]').getBoundingClientRect();
    return {x:Math.round(r.x),y:Math.round(r.y),width:Math.round(r.width),height:Math.round(r.height),
      padding:s.padding,radius:s.borderTopLeftRadius,background:s.backgroundColor,
      backdrop:getComputedStyle(d,'::backdrop').backgroundColor,modal:d.matches(':modal'),
      titleSize:getComputedStyle(h).fontSize,titleTextTop:Math.round(range.getBoundingClientRect().top),
      closeX:Math.round(r.right-c.right),closeY:Math.round(c.top-r.top),closeSize:Math.round(c.width),
      sectionY:Math.round(d.querySelector('[data-section]').getBoundingClientRect().top-r.top)}
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

describe('when the modal draws a side drawer', () => {
  it('should fill the height at the right edge of a phone, 85% wide, on the dark surface', async () => {
    const m = await open(390, 844)
    expect(m).toMatchObject({ x: 59, y: 0, width: 332, height: 844, modal: true })
    expect(m).toMatchObject({ padding: '24px', radius: '0px', background: 'rgb(22, 21, 24)' })
    expect(m.backdrop).toBe('rgba(0, 0, 0, 0.6)')
  })

  it('should put the title beside the close button and the content 24px below', async () => {
    const m = await open(390, 844)
    expect(m).toMatchObject({
      titleSize: '20px',
      closeX: 24,
      closeY: 24,
      closeSize: 32,
      sectionY: 80
    })
    // The 25px title line sits in the middle of the 32px close button's row.
    expect(m.titleTextTop).toBeGreaterThanOrEqual(27)
    expect(m.titleTextTop).toBeLessThanOrEqual(28)
  })

  it('should stop at 360px on a wider screen', async () => {
    const m = await open(1440, 900)
    expect(m).toMatchObject({ x: 1080, width: 360, height: 900 })
  })

  it('should collapse a hidden title to the visually hidden 1px box', async () => {
    await viewport(390, 844)
    await browser.session.navigate(`${browser.origin}?hidden=1`)
    const box = await browser.evaluate<number[]>(`(async()=>{
      for(let i=0;i<120&&!document.querySelector('dialog[open]');i++)await new Promise(r=>requestAnimationFrame(r));
      const r=document.querySelector('dialog h2').getBoundingClientRect();return [r.width,r.height]
    })()`)
    expect(box).toEqual([1, 1])
  })
})

describe('when the drawer holds the category Dropdown', () => {
  const state = () =>
    browser.evaluate<{ drawer: boolean; list: boolean }>(
      `(()=>{const d=document.querySelector('dialog[open]');const l=d&&d.querySelector('[role=listbox]');return {drawer:Boolean(d),list:Boolean(l)&&l.getBoundingClientRect().height>0}})()`
    )
  const press = async (key: string, code = 27) => {
    for (const type of ['keyDown', 'keyUp'] as const)
      await browser.session.send('Input.dispatchKeyEvent', {
        type,
        key,
        code: key,
        windowsVirtualKeyCode: code
      })
    await browser.evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
  }

  it('should close the list on the first Escape and the drawer on the second', async () => {
    await viewport(390, 844)
    await browser.session.navigate(`${browser.origin}?dropdown=1`)
    const at = await browser.evaluate<{ x: number; y: number }>(`(async()=>{
      for(let i=0;i<120&&!document.querySelector('dialog[open] [role=combobox]');i++)await new Promise(r=>requestAnimationFrame(r));
      const r=document.querySelector('dialog[open] [role=combobox]').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}
    })()`)
    for (const type of ['mousePressed', 'mouseReleased'] as const)
      await browser.session.send('Input.dispatchMouseEvent', {
        type,
        x: at.x,
        y: at.y,
        button: 'left',
        clickCount: 1
      })
    await browser.evaluate('new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)))')
    expect(await state()).toEqual({ drawer: true, list: true })
    await press('Escape')
    expect(await state()).toEqual({ drawer: true, list: false })
    await press('Escape')
    expect(await state()).toEqual({ drawer: false, list: false })
  })
})

async function viewport(width: number, height: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: width < 600
  })
}
