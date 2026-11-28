// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/blog-pills-browser.tsx')

type Box = {
  width: number
  height: number
  background: string
  backgroundImage: string
  boxShadow: string
  color: string
  fontSize: string
  fontWeight: string
  letterSpacing: string
  text: string
  inner: { x: number; y: number; width: number; height: number } | null
}

async function open(width: number) {
  await browser.session.send('Emulation.setDeviceMetricsOverride', {
    width,
    height: 900,
    deviceScaleFactor: 1,
    mobile: width < 768
  })
  await browser.session.navigate(browser.origin)
}

const box = (id: string) =>
  browser.evaluate<Box>(`(async()=>{
    for (let n = 0; n < 100 && !document.getElementById(${JSON.stringify(id)}); n++) await new Promise((r) => setTimeout(r, 20))
    await document.fonts.ready
    const el = document.getElementById(${JSON.stringify(id)})
    const r = el.getBoundingClientRect()
    const cs = getComputedStyle(el)
    const inner = el.querySelector('img') ?? el.querySelector('span')
    const text = el.querySelector('[role="img"]') ?? el.querySelector('span span') ?? el.querySelector('span')
    const ir = inner && inner.getBoundingClientRect()
    return {width:r.width,height:r.height,background:cs.backgroundColor,backgroundImage:cs.backgroundImage,boxShadow:cs.boxShadow,
      color:getComputedStyle(text ?? el).color,fontSize:getComputedStyle(text ?? el).fontSize,fontWeight:getComputedStyle(text ?? el).fontWeight,
      letterSpacing:getComputedStyle(text ?? el).letterSpacing,text:el.textContent,
      inner: ir ? {x:ir.x-r.x,y:ir.y-r.y,width:ir.width,height:ir.height} : null}
  })()`)

const gradient = 'linear-gradient(135deg, rgb(165, 36, 179), rgb(255, 45, 85))'

describe('when a ProfilePic has no ring and the gradient fallback', () => {
  it('should fill 56px with the picture and keep no ring', async () => {
    await open(1440)
    const pic = await box('pic-56')
    expect([pic.width, pic.height, pic.boxShadow, pic.backgroundImage]).toEqual([
      56,
      56,
      'none',
      gradient
    ])
    expect(pic.inner).toEqual({ x: 0, y: 0, width: 56, height: 56 })
  })

  it('should draw two 20px bold initials on the gradient when there is no picture', async () => {
    await open(1440)
    const pic = await box('pic-56-initials')
    expect([pic.text, pic.fontSize, pic.fontWeight, pic.color]).toEqual([
      'JD',
      '20px',
      '700',
      'rgb(252, 252, 252)'
    ])
  })

  it('should keep the 2px ring at 28px with a 24px picture over the gradient, and a blank disc without one', async () => {
    await open(390)
    const pic = await box('pic-28')
    expect([pic.width, pic.boxShadow, pic.backgroundImage]).toEqual([
      28,
      'rgba(255, 255, 255, 0.6) 0px 0px 0px 2px inset',
      gradient
    ])
    expect(pic.inner).toEqual({ x: 2, y: 2, width: 24, height: 24 })
    expect((await box('pic-28-blank')).text).toBe('')
  })

  it('should leave the default 72px picture with its ring and name colour', async () => {
    await open(1440)
    const pic = await box('pic-72-default')
    expect([pic.width, pic.boxShadow, pic.backgroundImage]).toEqual([
      72,
      'rgba(255, 255, 255, 0.6) 0px 0px 0px 3.6px inset',
      'none'
    ])
  })
})

describe('when a category Tag renders', () => {
  it.each([
    [390, 24],
    [900, 24],
    [1440, 26]
  ])(
    'should draw the medium pill %ipx wide at its own width and %ipx high',
    async (width, height) => {
      await open(width)
      const tag = await box('tag-md')
      expect(tag.height).toBe(height)
      expect(tag.width).toBeLessThan(320)
      expect([tag.background, tag.color, tag.fontSize, tag.fontWeight, tag.letterSpacing]).toEqual([
        'rgb(255, 116, 57)',
        'rgb(0, 0, 0)',
        '12px',
        '600',
        '0.5px'
      ])
    }
  )

  it('should draw the communities and the default about colours', async () => {
    await open(1440)
    const communities = await box('tag-communities')
    expect([communities.background, communities.color]).toEqual([
      'rgb(104, 13, 74)',
      'rgb(252, 252, 252)'
    ])
    const about = await box('tag-about')
    expect([about.background, about.color]).toEqual(['rgb(198, 64, 205)', 'rgb(0, 0, 0)'])
  })

  it('should draw the large pill 31px high with 16px bold capitals on its brand colour', async () => {
    await open(390)
    const tag = await box('tag-lg')
    expect([
      tag.height,
      tag.background,
      tag.color,
      tag.fontSize,
      tag.fontWeight,
      tag.letterSpacing
    ]).toEqual([31, 'rgb(255, 45, 85)', 'rgb(252, 252, 252)', '16px', '700', '0.64px'])
  })
})

describe('when colours are forced', () => {
  it('should draw every category pill in the system colours', async () => {
    await open(1440)
    await browser.session.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'forced-colors', value: 'active' }]
    })
    try {
      const system = await browser.evaluate<{ canvas: string; text: string }>(`(()=>{
        const probe = document.createElement('span')
        probe.style.forcedColorAdjust = 'none'
        document.body.append(probe)
        probe.style.color = 'Canvas'
        const canvas = getComputedStyle(probe).color
        probe.style.color = 'CanvasText'
        const text = getComputedStyle(probe).color
        probe.remove()
        return { canvas, text }
      })()`)
      expect(system.canvas).not.toBe(system.text)
      for (const id of ['tag-md', 'tag-communities', 'tag-about', 'tag-lg']) {
        const tag = await box(id)
        expect([id, tag.background, tag.color]).toEqual([id, system.canvas, system.text])
      }
    } finally {
      await browser.session.send('Emulation.setEmulatedMedia', { features: [] })
    }
  })
})
