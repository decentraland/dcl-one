// @vitest-environment node
import { beforeEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'

const browser = productionBrowser('test/consumer/context-submenu-browser.tsx')

type Box = { left: number; right: number; top: number; bottom: number }
// `next` is the menu's Block row, below the row that opens the list.
type State = { open: boolean; row: Box; next: Box; list: Box | null; focused: string | null }

// Two frames after the input, so React has committed what the pointer or key changed.
const read = (side: string) =>
  browser.evaluate<State>(`new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>{
    const root=document.getElementById(${JSON.stringify(side)})
    const row=root.querySelector('[aria-haspopup=menu]')
    const box=(el)=>{const r=el.getBoundingClientRect();return{left:r.left,right:r.right,top:r.top,bottom:r.bottom}}
    const id=row.getAttribute('aria-controls')
    const list=id?document.getElementById(id).querySelector('[role=menu]'):null
    const next=[...root.querySelectorAll('[role=menuitem]')].find(item=>item.textContent==='Block')
    resolve({open:row.getAttribute('aria-expanded')==='true',row:box(row),next:box(next),list:list?box(list):null,focused:document.activeElement?.textContent??null})
  })))`)

const move = (x: number, y: number) =>
  browser.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y })

async function key(name: 'Enter', code: number, text: string) {
  for (const type of ['keyDown', 'keyUp'] as const)
    await browser.session.send('Input.dispatchKeyEvent', {
      type,
      key: name,
      code: name,
      windowsVirtualKeyCode: code,
      text: type === 'keyDown' ? text : undefined
    })
}

beforeEach(async () => {
  await move(0, 0)
  await browser.session.navigate(browser.origin)
  await browser.evaluate(
    '(async()=>{for(let n=0;n<100&&!document.querySelector("#left [aria-haspopup]");n++)await new Promise(r=>setTimeout(r,20));await document.fonts.ready})()'
  )
})

describe.each([
  ['right', 1],
  ['left', -1]
] as const)('when a context menu row opens its list on the %s', (side, step) => {
  // Points at the row near the edge its list opens from, the way a pointer heads for the list.
  async function hover() {
    const { row } = await read(side)
    const y = Math.round((row.top + row.bottom) / 2)
    const x = Math.round(step > 0 ? row.right - 12 : row.left + 12)
    await move(x, y)
    return { x, y, state: await read(side) }
  }

  it(`should open the list on the row's ${side}, past the menu's edge`, async () => {
    const { state } = await hover()
    expect(state.open).toBe(true)
    if (step > 0) expect(state.list!.left).toBeGreaterThan(state.row.right)
    else expect(state.list!.right).toBeLessThan(state.row.left)
    expect(browser.session.runtimeErrors).toEqual([])
  })

  it('should keep the list open at every pixel the pointer crosses from the row to the list', async () => {
    const { x, y, state } = await hover()
    const target = Math.round(step > 0 ? state.list!.left + 8 : state.list!.right - 8)
    for (let at = x + step; step > 0 ? at <= target : at >= target; at += step) {
      await move(at, y)
      expect((await read(side)).open, `pointer at x=${at}`).toBe(true)
    }
  })

  it('should close the list when the pointer leaves for another row', async () => {
    const { state } = await hover()
    await move(
      Math.round((state.next.left + state.next.right) / 2),
      Math.round((state.next.top + state.next.bottom) / 2)
    )
    const after = await read(side)
    expect(after.open).toBe(false)
    expect(after.list).toBeNull()
  })

  it(`should open from the keyboard on the row's ${side} too, with the focus on the list's first item`, async () => {
    await browser.evaluate(
      `document.getElementById(${JSON.stringify(side)}).querySelector('[aria-haspopup=menu]').focus()`
    )
    await key('Enter', 13, '\r')
    const state = await read(side)
    expect(state.open).toBe(true)
    expect(state.focused).toBe('Pixel Builders')
    if (step > 0) expect(state.list!.left).toBeGreaterThan(state.row.right)
    else expect(state.list!.right).toBeLessThan(state.row.left)
  })
})
