// @vitest-environment node
import { beforeEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/pagination-dots-browser.tsx')

type Dot = {
  x: number
  y: number
  width: number
  height: number
  background: string
  current: string | null
  target: { width: number; height: number; left: number; top: number }
}

const dots = (row: string) =>
  browser.evaluate<Dot[]>(`(async()=>{
    for (let n = 0; n < 100 && !document.querySelector('#${row} button'); n++) await new Promise((r) => setTimeout(r, 20))
    return Array.from(document.querySelectorAll('#${row} button')).map((b)=>{
      const r=b.getBoundingClientRect()
      const after=getComputedStyle(b,'::after')
      const width=parseFloat(after.width), height=parseFloat(after.height)
      return {x:r.x,y:r.y,width:r.width,height:r.height,background:getComputedStyle(b).backgroundColor,current:b.getAttribute('aria-current'),
        target:{width,height,left:r.x+parseFloat(after.left),top:r.y+parseFloat(after.top)}}
    })
  })()`)

const press = async (dot: Dot) => {
  for (const type of ['mousePressed', 'mouseReleased'] as const)
    await browser.session.send('Input.dispatchMouseEvent', {
      type,
      x: dot.target.left + 1,
      y: dot.target.top + 1,
      button: 'left',
      clickCount: 1
    })
}

beforeEach(async () => {
  await browser.session.send('Emulation.setEmulatedMedia', {
    features: [{ name: 'prefers-reduced-motion', value: 'reduce' }]
  })
  await browser.session.navigate(browser.origin)
})

describe('when the default dots render', () => {
  it('should draw 14px dots 8px apart and a 40px ruby pill for the current page', async () => {
    const row = await dots('default')
    expect(row.map((d) => [d.width, d.height])).toEqual([
      [14, 14],
      [40, 14],
      [14, 14],
      [14, 14]
    ])
    expect(row[1]!.x - (row[0]!.x + row[0]!.width)).toBe(8)
    expect(row.map((d) => d.background)).toEqual([
      'rgba(255, 255, 255, 0.2)',
      'rgb(255, 45, 85)',
      'rgba(255, 255, 255, 0.2)',
      'rgba(255, 255, 255, 0.2)'
    ])
  })
})

describe('when the plain dots render', () => {
  it('should draw 8px dots on a 16px pitch, the current one white', async () => {
    const row = await dots('plain')
    expect(row.map((d) => [d.width, d.height])).toEqual(Array(4).fill([8, 8]))
    expect(row.slice(1).map((d, i) => d.x - row[i]!.x)).toEqual([16, 16, 16])
    expect(row.map((d) => d.background)).toEqual([
      'rgba(255, 255, 255, 0.4)',
      'rgb(255, 255, 255)',
      'rgba(255, 255, 255, 0.4)',
      'rgba(255, 255, 255, 0.4)'
    ])
  })

  it('should give each dot a 24px high target that meets its neighbours without overlapping them', async () => {
    const row = await dots('plain')
    for (const dot of row) expect([dot.target.width, dot.target.height]).toEqual([16, 24])
    for (let i = 1; i < row.length; i++)
      expect(row[i]!.target.left).toBe(row[i - 1]!.target.left + row[i - 1]!.target.width)
  })

  it('should move the current page to a dot pressed at the edge of its target', async () => {
    const row = await dots('plain')
    await press(row[3]!)
    expect((await dots('plain')).map((d) => d.current)).toEqual([null, null, null, 'true'])
  })
})
