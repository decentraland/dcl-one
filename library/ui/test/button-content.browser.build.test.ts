// @vitest-environment node
import { beforeEach, afterEach, describe, it, expect } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
import { productionBrowser } from './production-browser.helpers'
const b = productionBrowser('test/consumer/button-content.tsx')
async function ready() {
  const errorStart = b.session.runtimeErrors.length
  await b.session.navigate(b.origin)
  await waitForFixture(
    'Button fill fixture',
    () =>
      b.evaluate<boolean>(
        `Boolean(document.querySelector('[data-case="fill"]')?.getBoundingClientRect().width && document.fonts.status === 'loaded')`
      ),
    Boolean,
    () => b.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
    () => b.session.runtimeErrors.slice(errorStart)
  )
}
async function activate(selector: string) {
  const pt = await b.evaluate<{ x: number; y: number }>(
    `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`
  )
  for (const type of ['mousePressed', 'mouseReleased'])
    await b.session.send('Input.dispatchMouseEvent', { type, ...pt, button: 'left', clickCount: 1 })
}
describe('when Button owns content layout in a production bundle', () => {
  let caseErrorStart = 0
  beforeEach(() => {
    caseErrorStart = b.session.runtimeErrors.length
  })
  afterEach(() => {
    expect(b.session.runtimeErrors.slice(caseErrorStart)).toEqual([])
  })
  it('should preserve intrinsic shrink wrapping and place fill icons at both content edges', async () => {
    await ready()
    const layout = await b.evaluate<
      { button: number; content: number; left: number; right: number; align: string }[]
    >(
      `['intrinsic','fill'].map(name=>{const n=document.querySelector('[data-case="'+name+'"]'),c=n.querySelector('[data-content-layout]'),r=n.getBoundingClientRect(),a=c.getBoundingClientRect(),left=n.querySelector('[data-marker="leading"]').getBoundingClientRect(),right=n.querySelector('[data-marker="trailing"]').getBoundingClientRect();return{button:r.width,content:a.width,left:left.left-a.left,right:a.right-right.right,align:getComputedStyle(n.querySelector('[data-button-label]')).textAlign}})`
    )
    expect(layout.map((n) => n.button)).toEqual([230, 230])
    expect(layout[0]!.content).toBeLessThan(layout[1]!.content)
    expect(layout[1]!.left).toBe(0)
    expect(layout[1]!.right).toBe(0)
    expect(layout[1]!.align).toBe('start')
  })
  it('should bound long labels without hiding fixed icons or losing complete text', async () => {
    await ready()
    const data = await b.evaluate<{
      root: number
      label: number
      scroll: number
      overflow: string
      left: number
      right: number
      text: string
    }>(
      `(()=>{const n=document.querySelector('[data-case="long"]'),l=n.querySelector('[data-button-label]'),r=n.getBoundingClientRect();return{root:r.width,label:l.clientWidth,scroll:l.scrollWidth,overflow:getComputedStyle(l).textOverflow,left:n.querySelector('[data-marker="leading"]').getBoundingClientRect().width,right:n.querySelector('[data-marker="trailing"]').getBoundingClientRect().width,text:n.textContent}})()`
    )
    expect(data.root).toBe(230)
    expect(data.scroll).toBeGreaterThan(data.label)
    expect(data.overflow).toBe('ellipsis')
    expect([data.left, data.right]).toEqual([20, 20])
    expect(data.text).toContain('covering either icon')
  })
  describe('and fill is requested on a compact button', () => {
    it('should preserve the existing compact 48px geometry and intrinsic centering', async () => {
      await ready()
      expect(
        await b.evaluate(
          `(()=>{const n=document.querySelector('[data-case="compact"]'),r=n.getBoundingClientRect(),i=n.querySelector('[data-marker]').getBoundingClientRect();return[r.width,r.height,i.left+i.width/2-r.left-r.width/2]})()`
        )
      ).toEqual([48, 48, 0])
    })
  })
  describe('and a loading button is activated twice', () => {
    it('should keep the loading frame/name and accept one caller activation only', async () => {
      await ready()
      const before = await b.evaluate<number[]>(
        `(()=>{const r=document.querySelector('[data-case="loading"]').getBoundingClientRect();return[r.width,r.height]})()`
      )
      await activate('[data-case="loading"]')
      await activate('[data-case="loading"]')
      expect(await b.evaluate('document.querySelector("output").textContent')).toBe('1')
      expect(
        await b.evaluate(
          `(()=>{const n=document.querySelector('[data-case="loading"]'),r=n.getBoundingClientRect();return[r.width,r.height,n.textContent,getComputedStyle(n.querySelector('[data-content-layout]')).opacity]})()`
        )
      ).toEqual([...before, 'Save record', '0'])
    })
  })
  describe('and a link button receives a trusted Enter', () => {
    it('should preserve native anchor focus and destination', async () => {
      await ready()
      await b.evaluate(`document.querySelector('[data-case="link"]').focus()`)
      expect(await b.evaluate('document.activeElement.tagName')).toBe('A')
      for (const type of ['keyDown', 'keyUp'])
        await b.session.send('Input.dispatchKeyEvent', {
          type,
          key: 'Enter',
          code: 'Enter',
          windowsVirtualKeyCode: 13,
          text: type === 'keyDown' ? '\r' : undefined
        })
      expect(await b.evaluate('location.hash')).toBe('#target')
      expect(await b.evaluate(`document.querySelector('[data-case="link"]').tagName`)).toBe('A')
    })
  })
})
