// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
import { waitForFixture } from './fixture-state.helpers'
import type { CheckboxMetrics } from './consumer/checkbox-browser'

const browser = productionBrowser('test/consumer/checkbox-browser.tsx')

// Values from the Style Guide `checkbox` set and the foundations they resolve to.
const primary = 'rgb(255, 45, 85)'
const primaryHover = 'rgb(255, 74, 108)'
const primaryPressed = 'rgb(216, 0, 41)'
const primarySubtle = 'rgba(255, 45, 85, 0.2)'
const disabledFill = 'rgb(67, 64, 74)'
const disabledMark = 'rgb(113, 107, 124)'
const snow = 'rgb(252, 252, 252)'
const gray = 'rgb(207, 205, 212)'
const white10 = 'rgba(255, 255, 255, 0.1)'
const white40 = 'rgba(255, 255, 255, 0.4)'
const white60 = 'rgba(255, 255, 255, 0.6)'
const none = 'rgba(0, 0, 0, 0)'

let errorStart = 0
const errors = () => browser.session.runtimeErrors.slice(errorStart)
const nextFrame = () =>
  browser.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))')
const read = (id: string) =>
  browser.evaluate<CheckboxMetrics>(`window.checkboxProbe.metrics(${JSON.stringify(id)})`)

async function settle(id: string, expected: Partial<CheckboxMetrics>) {
  const value = await waitForFixture(
    `${id} ${JSON.stringify(expected)}`,
    () => read(id),
    (m) =>
      Object.entries(expected).every(([key, want]) => m[key as keyof CheckboxMetrics] === want),
    nextFrame,
    errors
  )
  expect(value).toMatchObject(expected)
  return value
}

async function ready(query = '') {
  errorStart = browser.session.runtimeErrors.length
  await browser.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
  await browser.session.navigate(browser.origin + query)
  await waitForFixture(
    'Checkbox fixture',
    () =>
      browser.evaluate<boolean>(
        `Boolean(document.getElementById('outline-md-off')&&document.fonts.status==='loaded')`
      ),
    Boolean,
    nextFrame,
    errors
  )
}

async function mouse(type: 'mouseMoved' | 'mousePressed' | 'mouseReleased', at: CheckboxMetrics) {
  await browser.session.send('Input.dispatchMouseEvent', {
    type,
    x: at.x,
    y: at.y,
    ...(type === 'mouseMoved' ? {} : { button: 'left', clickCount: 1 })
  })
}

beforeEach(() => ready())
afterEach(async () => {
  try {
    expect(errors()).toEqual([])
  } finally {
    await browser.session.send('Emulation.setTouchEmulationEnabled', { enabled: false })
    await browser.session.send('Emulation.setEmulatedMedia', { features: [] })
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false
    })
  }
})

describe('when the checkbox renders on a desktop pointer', () => {
  it.each([
    ['sm', 28, 16, 12, '4px'],
    ['md', 32, 20, 16, '6px'],
    ['lg', 36, 24, 20, '6px']
  ] as const)(
    'should draw the %s box at the design size',
    async (size, target, box, glyph, radius) => {
      const m = await settle(`outline-${size}-on`, { check: true, haloSize: target, haloOffset: 0 })
      expect([m.target, m.box, m.glyph, m.radius, m.borderWidth]).toEqual([
        target,
        box,
        glyph,
        radius,
        '1.5px'
      ])
    }
  )

  it('should draw the outline states with their design colours', async () => {
    await settle('outline-md-off', { fill: none, border: white40, check: false, dash: false })
    await settle('outline-md-on', { fill: primary, mark: snow, check: true, dash: false })
    await settle('outline-md-mixed', { fill: primary, mark: snow, check: false, dash: true })
    await settle('outline-md-off-disabled', { fill: none, border: disabledFill })
    await settle('outline-md-on-disabled', { fill: disabledFill, mark: disabledMark, check: true })
    await settle('outline-md-mixed-disabled', { fill: disabledFill, mark: snow, dash: true })
  })

  it('should draw the filled states with their design colours', async () => {
    await settle('filled-md-off', { fill: snow, border: gray, check: false })
    await settle('filled-md-on', { fill: snow, border: gray, mark: primary, check: true })
    await settle('filled-md-mixed', { fill: snow, border: gray, mark: primary, dash: true })
    await settle('filled-md-off-disabled', { fill: disabledFill, border: gray })
    await settle('filled-md-on-disabled', { fill: disabledFill, border: gray, mark: disabledMark })
    await settle('filled-md-mixed-disabled', { fill: disabledFill, border: gray, mark: snow })
  })

  it.each([
    ['outline-md-off', { border: white60, halo: white10 }],
    ['outline-md-on', { fill: primaryHover, halo: primarySubtle }],
    ['outline-md-mixed', { fill: primaryHover, halo: primarySubtle }],
    ['filled-md-off', { fill: snow, border: gray, halo: white10 }],
    ['filled-md-on', { fill: snow, border: gray, halo: primarySubtle }]
  ] as const)('should show the hover state of %s under the pointer', async (id, hover) => {
    const at = await settle(id, { halo: none })
    await mouse('mouseMoved', at)
    await settle(id, hover)
  })

  it.each([
    ['outline-md-off', { border: snow, halo: white10 }, true],
    ['outline-md-on', { fill: primaryPressed, halo: primarySubtle }, false],
    ['filled-md-off', { fill: gray, border: gray, halo: white10 }, true],
    ['filled-md-on', { fill: gray, border: gray, halo: primarySubtle }, false]
  ] as const)(
    'should show the pressed state of %s and toggle on release',
    async (id, pressed, after) => {
      const at = await read(id)
      await mouse('mouseMoved', at)
      try {
        await mouse('mousePressed', at)
        await settle(id, pressed)
      } finally {
        await mouse('mouseReleased', at)
      }
      await settle(id, { checked: after })
    }
  )

  it('should show no hover or pressed state and stay unchanged when disabled', async () => {
    const at = await read('outline-md-on-disabled')
    await mouse('mouseMoved', at)
    await mouse('mousePressed', at)
    await mouse('mouseReleased', at)
    await settle('outline-md-on-disabled', { halo: none, fill: disabledFill, checked: true })
  })

  it('should keep the mixed state the caller set after a click', async () => {
    const at = await read('outline-md-mixed')
    await mouse('mouseMoved', at)
    await mouse('mousePressed', at)
    await mouse('mouseReleased', at)
    await settle('outline-md-mixed', { indeterminate: true, dash: true })
  })
})

describe('when the reader uses the keyboard', () => {
  it('should ring the box on keyboard focus and toggle it with Space', async () => {
    await browser.evaluate(`document.getElementById('outline-sm-off').focus({focusVisible:true})`)
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Tab',
      code: 'Tab'
    })
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Tab', code: 'Tab' })
    const id = await browser.evaluate<string>('document.activeElement.id')
    expect(id).toBe('outline-sm-on')
    await settle(id, { outline: `solid 2px ${snow}` })
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: ' ',
      code: 'Space',
      text: ' '
    })
    await browser.session.send('Input.dispatchKeyEvent', { type: 'keyUp', key: ' ', code: 'Space' })
    await settle(id, { checked: false, check: false })
  })

  it('should skip a disabled checkbox when tabbing', async () => {
    await browser.evaluate(`document.getElementById('outline-lg-mixed').focus()`)
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Tab',
      code: 'Tab'
    })
    expect(await browser.evaluate<string>('document.activeElement.id')).toBe('filled-sm-off')
  })

  it('should draw no ring after a mouse click', async () => {
    const at = await read('outline-md-off')
    await mouse('mouseMoved', at)
    await mouse('mousePressed', at)
    await mouse('mouseReleased', at)
    const m = await settle('outline-md-off', { checked: true })
    expect(m.outline.startsWith('none')).toBe(true)
  })
})

describe('when the pointer is coarse', () => {
  it('should widen every target to 44px, keep the box sizes and drop hover', async () => {
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
    await ready()
    expect(await browser.evaluate(`matchMedia('(pointer:coarse)').matches`)).toBe(true)
    for (const [size, box, halo] of [
      ['sm', 16, 28],
      ['md', 20, 32],
      ['lg', 24, 36]
    ] as const) {
      const m = await read(`filled-${size}-on`)
      // Figma viewport=mobile keeps the desktop halo, centred in the 44px target.
      expect([m.target, m.box, m.haloSize, m.haloOffset]).toEqual([44, box, halo, (44 - halo) / 2])
    }
    expect(await browser.evaluate(`matchMedia('(hover:hover)').matches`)).toBe(false)
  })
})

describe('when the system forces colours or reduces motion', () => {
  it('should draw checked boxes in the system highlight and hide the halo', async () => {
    await browser.session.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'forced-colors', value: 'active' }]
    })
    const system = await browser.evaluate<Record<string, string>>(`(()=>{
      const probe=document.createElement('span')
      probe.style.forcedColorAdjust='none'
      document.body.append(probe)
      const read=(name)=>{probe.style.color=name;return getComputedStyle(probe).color}
      const colors={Canvas:read('Canvas'),CanvasText:read('CanvasText'),Highlight:read('Highlight'),HighlightText:read('HighlightText'),GrayText:read('GrayText')}
      probe.remove()
      return colors
    })()`)
    expect(system.Highlight).not.toBe(system.Canvas)
    // Settled, because the colour change from the normal palette runs through the transition.
    await settle('outline-md-on', {
      haloDisplay: 'none',
      fill: system.Highlight,
      mark: system.HighlightText,
      check: true
    })
    await settle('outline-md-off', { fill: system.Canvas, mark: system.CanvasText })
    for (const id of [
      'outline-md-on-disabled',
      'outline-md-mixed-disabled',
      'filled-md-on-disabled'
    ])
      // A disabled box never takes the selection colour.
      await settle(id, { fill: system.Canvas, mark: system.GrayText })
  })

  it('should drop the transitions', async () => {
    await browser.session.send('Emulation.setEmulatedMedia', {
      features: [{ name: 'prefers-reduced-motion', value: 'reduce' }]
    })
    expect((await read('outline-md-on')).transition).toBe('0s')
  })
})

describe('when the checkbox has a label', () => {
  type Row = {
    box: DOMRectLike
    text: DOMRectLike
    help: DOMRectLike | null
    font: string
    lineHeight: string
    helpColor: string | null
  }
  type DOMRectLike = { x: number; y: number; width: number; height: number }
  const row = (id: string) =>
    browser.evaluate<Row>(`(()=>{
      const input=document.getElementById(${JSON.stringify(id)})
      const root=input.parentElement.parentElement
      const r=(el)=>{if(!el)return null;const b=el.getBoundingClientRect();return{x:b.x,y:b.y,width:b.width,height:b.height}}
      const label=root.querySelector('label')
      const help=root.querySelector('button svg')
      return{box:r(input.parentElement),text:r(label),help:r(help),font:getComputedStyle(label).fontSize,lineHeight:getComputedStyle(label).lineHeight,helpColor:help?getComputedStyle(help).color:null}
    })()`)

  it('should place the box 4px before the text at the end position', async () => {
    const m = await row('label-end')
    expect(m.text.x - (m.box.x + m.box.width)).toBeCloseTo(4, 0)
    expect(m.help!.x - (m.text.x + m.text.width)).toBeCloseTo(4, 0)
    expect([m.font, m.lineHeight, m.help!.width, m.helpColor]).toEqual([
      '14px',
      'normal',
      18,
      white40
    ])
  })

  it('should place the box 8px after the help at the start position', async () => {
    const m = await row('label-start')
    expect(m.box.x - (m.help!.x + m.help!.width)).toBeCloseTo(8, 0)
    expect(m.text.x).toBeLessThan(m.box.x)
  })

  it('should toggle from the text, and open the help without toggling', async () => {
    const m = await row('label-end')
    await browser.session.send('Input.dispatchMouseEvent', {
      type: 'mousePressed',
      x: m.help!.x + 9,
      y: m.help!.y + 9,
      button: 'left',
      clickCount: 1
    })
    await browser.session.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased',
      x: m.help!.x + 9,
      y: m.help!.y + 9,
      button: 'left',
      clickCount: 1
    })
    await settle('label-end', { checked: false })
    const bubble = await browser.evaluate<{
      visible: boolean
      top: number
      left: number
      right: number
      expanded: string | null
    }>(`(()=>{
      const root=document.getElementById('label-end').parentElement.parentElement
      const button=root.querySelector('button')
      const text=document.getElementById(button.getAttribute('aria-controls'))
      const r=text.getBoundingClientRect()
      return{visible:!text.hidden&&r.width>0,top:r.top,left:r.left,right:r.right,expanded:button.getAttribute('aria-expanded')}
    })()`)
    expect(bubble).toMatchObject({ visible: true, expanded: 'true' })
    expect(bubble.top).toBeGreaterThan(m.help!.y + m.help!.height)
    expect(bubble.left).toBeGreaterThanOrEqual(0)
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyDown',
      key: 'Escape',
      code: 'Escape'
    })
    await browser.session.send('Input.dispatchKeyEvent', {
      type: 'keyUp',
      key: 'Escape',
      code: 'Escape'
    })
    expect(
      await browser.evaluate<string>(
        `document.getElementById('label-end').parentElement.parentElement.querySelector('button').getAttribute('aria-expanded')`
      )
    ).toBe('false')
    for (const type of ['mousePressed', 'mouseReleased'] as const)
      await browser.session.send('Input.dispatchMouseEvent', {
        type,
        x: m.text.x + 10,
        y: m.text.y + m.text.height / 2,
        button: 'left',
        clickCount: 1
      })
    await settle('label-end', { checked: true })
  })

  it('should wrap a long label inside a narrow column without moving the box', async () => {
    const m = await row('label-wrap')
    expect(m.text.height).toBeGreaterThan(20)
    expect(m.box.width).toBe(32)
    const column = await browser.evaluate<number>(
      `document.getElementById('label-wrap').closest('div').getBoundingClientRect().right`
    )
    expect(m.help!.x + m.help!.width).toBeLessThanOrEqual(column)
  })
})

async function pressHelp(id: string) {
  const at = await browser.evaluate<{ x: number; y: number }>(`(()=>{
    const button=document.getElementById(${JSON.stringify(id)}).parentElement.parentElement.querySelector('button')
    button.scrollIntoView({block:'center',inline:'nearest'})
    const r=button.getBoundingClientRect()
    return{x:r.x+r.width/2,y:r.y+r.height/2}
  })()`)
  for (const type of ['mousePressed', 'mouseReleased'] as const)
    await browser.session.send('Input.dispatchMouseEvent', {
      type,
      ...at,
      button: 'left',
      clickCount: 1
    })
}

async function escape() {
  for (const type of ['keyDown', 'keyUp'] as const)
    await browser.session.send('Input.dispatchKeyEvent', {
      type,
      key: 'Escape',
      code: 'Escape',
      windowsVirtualKeyCode: 27,
      nativeVirtualKeyCode: 27
    })
}

describe('when the help opens near a viewport edge', () => {
  it.each([320, 1440])('should keep the open text fully inside a %ipx viewport', async (width) => {
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width,
      height: 700,
      deviceScaleFactor: 1,
      mobile: false
    })
    await ready('?edges')
    for (const [id, side] of [
      ['label-edge-top', 'below'],
      ['label-edge-right', 'above'],
      ['label-edge-left', 'above']
    ] as const) {
      await pressHelp(id)
      const open = await browser.evaluate<{
        expanded: string
        text: { left: number; right: number; top: number; bottom: number }
        button: { top: number; bottom: number }
        width: number
        height: number
      }>(`(()=>{
        const button=document.getElementById(${JSON.stringify(id)}).parentElement.parentElement.querySelector('button')
        const r=document.getElementById(button.getAttribute('aria-controls')).getBoundingClientRect()
        const b=button.getBoundingClientRect()
        return{expanded:button.getAttribute('aria-expanded'),text:{left:r.left,right:r.right,top:r.top,bottom:r.bottom},button:{top:b.top,bottom:b.bottom},width:document.documentElement.clientWidth,height:document.documentElement.clientHeight}
      })()`)
      expect(open.expanded).toBe('true')
      expect(open.text.left).toBeGreaterThanOrEqual(8)
      expect(open.text.right).toBeLessThanOrEqual(open.width - 8)
      expect(open.text.top).toBeGreaterThanOrEqual(8)
      expect(open.text.bottom).toBeLessThanOrEqual(open.height - 8)
      if (side === 'below') expect(open.text.top).toBeGreaterThan(open.button.bottom)
      else expect(open.text.bottom).toBeLessThan(open.button.top)
      await escape()
    }
  })
})

describe('when the help is inside a modal dialog', () => {
  it.each([320, 1440])('should show the label and the full help text at %ipx', async (width) => {
    await browser.session.send('Emulation.setDeviceMetricsOverride', {
      width,
      height: 700,
      deviceScaleFactor: 1,
      mobile: false
    })
    await browser.evaluate(`document.getElementById('open-dialog').click()`)
    await pressHelp('label-dialog')
    const result = await browser.evaluate(`(()=>{
      const dialog = document.getElementById('help-dialog')
      const button = dialog.querySelector('button')
      const bubble = document.getElementById(button.getAttribute('aria-controls'))
      const range = document.createRange()
      range.selectNodeContents(bubble)
      const lines = Array.from(range.getClientRects())
      return {
        background: getComputedStyle(dialog).backgroundColor,
        label: getComputedStyle(dialog.querySelector('label')).color,
        readable: lines.length > 0 && lines.every(line => [line.left + 1, line.right - 1].every(x =>
          bubble.contains(document.elementFromPoint(x, (line.top + line.bottom) / 2))))
      }
    })()`)
    expect(result).toEqual({ background: 'rgb(45, 42, 51)', label: snow, readable: true })
  })

  it('should close only the help on the first Escape and the dialog on the second', async () => {
    await browser.evaluate(`document.getElementById('open-dialog').click()`)
    const state = () =>
      browser.evaluate<{ dialog: boolean; expanded: string; focusOnHelp: boolean }>(`(()=>{
        const button=document.getElementById('label-dialog').parentElement.parentElement.querySelector('button')
        return{dialog:document.getElementById('help-dialog').open,expanded:button.getAttribute('aria-expanded'),focusOnHelp:document.activeElement===button}
      })()`)
    expect((await state()).dialog).toBe(true)
    await pressHelp('label-dialog')
    expect(await state()).toMatchObject({ dialog: true, expanded: 'true' })
    await escape()
    expect(await state()).toEqual({ dialog: true, expanded: 'false', focusOnHelp: true })
    await escape()
    expect((await state()).dialog).toBe(false)
  })
})
