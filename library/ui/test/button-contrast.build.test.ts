import { afterEach, beforeEach, describe, it, expect } from 'vitest'
import { productionBrowser } from './production-browser.helpers'
import { colorContrast, requiredTextContrast } from './color-contrast.helpers'
import { waitForFixture } from './fixture-state.helpers'
import type { ButtonMetrics as Metrics, TouchFeedback } from './consumer/button-metrics'
const browser = productionBrowser('test/consumer/button-contrast.tsx')
const sizes = ['xl', 'lg', 'md', 'sm'] as const
const contexts = sizes.flatMap((size) =>
  ['desktop', 'mobile'].map((viewport) => ({ size, viewport }))
)
let caseErrorStart = 0
let navigationErrorStart = 0
let expectedRuntimeError: string | undefined
beforeEach(() => {
  caseErrorStart = browser.session.runtimeErrors.length
  expectedRuntimeError = undefined
})
const navigationErrors = () => browser.session.runtimeErrors.slice(navigationErrorStart)
const nextFrame = () =>
  browser.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))')
async function ready(query: string) {
  navigationErrorStart = browser.session.runtimeErrors.length
  await browser.session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
  await browser.session.navigate(browser.origin + query)
  await waitForFixture(
    'Button fixture',
    () =>
      browser.evaluate<boolean>(
        `(()=>{const target=document.querySelector('#target');return Boolean(target&&target.getBoundingClientRect().width>0&&target.getBoundingClientRect().height>0&&document.fonts.status==='loaded')})()`
      ),
    Boolean,
    nextFrame,
    navigationErrors
  )
}
async function nativeState(expected: Partial<Pick<Metrics, 'background' | 'hovered' | 'pressed'>>) {
  const result = await waitForFixture(
    'Button native state ' + JSON.stringify(expected),
    metrics,
    (value) =>
      Object.entries(expected).every(([key, wanted]) => value[key as keyof Metrics] === wanted),
    nextFrame,
    navigationErrors
  )
  expect(result).toMatchObject(expected)
  return result
}
async function actionCount() {
  return waitForFixture(
    'Button activation',
    () => browser.evaluate<string | null>(`document.querySelector('output')?.textContent??null`),
    (value) => value === '1',
    nextFrame,
    navigationErrors
  )
}
async function metrics() {
  return browser.evaluate<Metrics>('window.buttonProbe.metrics()')
}
async function touchFeedback() {
  // Touch activation feedback can begin at the trusted compatibility mousedown after touchEnd.
  const sample = await waitForFixture(
    'Native touch activation feedback',
    () => browser.evaluate<TouchFeedback | null>('window.buttonProbe.touchFeedback'),
    (value) =>
      Boolean(
        value?.trusted &&
        value.fromTouch &&
        value.pointerType === 'touch' &&
        value.metrics.pressed &&
        value.metrics.background === 'rgb(216, 0, 41)'
      ),
    nextFrame,
    navigationErrors
  )
  expect(sample).toMatchObject({
    trusted: true,
    fromTouch: true,
    pointerType: 'touch',
    metrics: { pressed: true, background: 'rgb(216, 0, 41)' }
  })
  return sample!.metrics
}
async function key(key: string, shift = false) {
  for (const type of ['keyDown', 'keyUp'])
    await browser.session.send('Input.dispatchKeyEvent', {
      type,
      key,
      code: key,
      windowsVirtualKeyCode: key === 'Tab' ? 9 : key === 'Enter' ? 13 : 32,
      modifiers: shift ? 8 : 0,
      text:
        type === 'keyDown' ? (key === ' ' ? ' ' : key === 'Enter' ? '\r' : undefined) : undefined
    })
}
async function pointer(type: 'mouseMoved' | 'mousePressed' | 'mouseReleased', m: Metrics) {
  await browser.session.send('Input.dispatchMouseEvent', {
    type,
    x: m.x + m.width / 2,
    y: m.y + m.height / 2,
    ...(type === 'mouseMoved' ? {} : { button: 'left', clickCount: 1 })
  })
}
function contrast(m: Metrics) {
  return colorContrast(m.color, m.background)
}
afterEach(async () => {
  if (browser.session) {
    try {
      const errors = browser.session.runtimeErrors.slice(caseErrorStart)
      if (expectedRuntimeError)
        expect(errors).toEqual([expect.stringContaining(expectedRuntimeError)])
      else expect(errors).toEqual([])
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
  }
})
describe('when the shared primary button preserves its design foreground', () => {
  it.each(contexts)(
    'should retain white normal, hover and pressed text for $size $viewport',
    async (context) => {
      await ready('?size=' + context.size + '&viewport=' + context.viewport)
      const normal = await nativeState({
        background: 'rgb(255, 45, 85)',
        hovered: false,
        pressed: false
      })
      await pointer('mouseMoved', normal)
      const hover = await nativeState({
        background: 'rgb(255, 74, 108)',
        hovered: true,
        pressed: false
      })
      let pressed: Metrics
      try {
        await pointer('mousePressed', normal)
        pressed = await nativeState({ background: 'rgb(216, 0, 41)', pressed: true })
      } finally {
        await pointer('mouseReleased', normal)
      }
      for (const m of [normal, hover, pressed]) {
        expect(m.color).toBe('rgb(255, 255, 255)')
        if (m.background === 'rgb(216, 0, 41)')
          expect(contrast(m)).toBeGreaterThanOrEqual(requiredTextContrast(m.fontSize, m.fontWeight))
        expect([m.width, m.height]).toEqual([normal.width, normal.height])
      }
      expect(await actionCount()).toBe('1')
    }
  )
  it.each(sizes)(
    'should preserve design-white coarse input and a real native action for %s',
    async (size) => {
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
      await ready('?size=' + size + '&viewport=mobile')
      expect(
        await browser.evaluate(
          `matchMedia('(pointer:coarse)').matches && navigator.maxTouchPoints===1`
        )
      ).toBe(true)
      const normal = await nativeState({ background: 'rgb(255, 45, 85)', pressed: false })
      let touchOpen = true
      let pressed: Metrics
      try {
        await browser.session.send('Input.dispatchTouchEvent', {
          type: 'touchStart',
          touchPoints: [{ x: normal.x + normal.width / 2, y: normal.y + normal.height / 2, id: 1 }]
        })
        await browser.session.send('Input.dispatchTouchEvent', {
          type: 'touchEnd',
          touchPoints: []
        })
        touchOpen = false
        pressed = await touchFeedback()
      } finally {
        if (touchOpen)
          await browser.session.send('Input.dispatchTouchEvent', {
            type: 'touchCancel',
            touchPoints: []
          })
      }
      for (const m of [normal, pressed]) expect(m.color).toBe('rgb(255, 255, 255)')
      expect(await actionCount()).toBe('1')
    }
  )
  it('should record the normal-text contrast limitation of design-white primary paint', async () => {
    await ready('?size=md')
    const normal = await metrics()
    expect(normal.color).toBe('rgb(255, 255, 255)')
    expect(requiredTextContrast(normal.fontSize, normal.fontWeight)).toBe(4.5)
    expect(contrast(normal)).toBeCloseTo(3.6467641009, 9)
    expect(contrast(normal)).toBeLessThan(4.5)
    await pointer('mouseMoved', normal)
    const hover = await nativeState({ background: 'rgb(255, 74, 108)', hovered: true })
    expect(hover.color).toBe('rgb(255, 255, 255)')
    expect(contrast(hover)).toBeLessThan(4.5)
  })
  it.each(['', 'hover', 'pressed'])(
    'should preserve a caller-owned currentColor icon in the %s design state',
    async (state) => {
      await ready('?icon&force=' + state)
      expect((await metrics()).color).toBe('rgb(255, 255, 255)')
      expect(
        await browser.evaluate("getComputedStyle(document.querySelector('#target svg path')).fill")
      ).toBe('rgb(255, 255, 255)')
    }
  )
  it.each(['dark', 'light'])(
    'should retain real keyboard focus paint on the %s host without moving its target',
    async (theme) => {
      await ready('?theme=' + theme)
      const before = await metrics()
      await key('Tab')
      await key('Tab')
      const focused = await metrics()
      expect(focused.focused).toBe(true)
      expect([focused.width, focused.height]).toEqual([before.width, before.height])
      const png = (await browser.session.send('Page.captureScreenshot', { format: 'png' })) as {
        data: string
      }
      const pixels = await browser.evaluate<number[][]>(
        `(async()=>{const image=new Image();image.src='data:image/png;base64,${png.data}';await image.decode();const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;const ctx=canvas.getContext('2d');ctx.drawImage(image,0,0);return[${Math.floor(focused.x - 2)},${Math.floor(focused.x - 4)},${Math.floor(focused.x - 8)}].map(x=>Array.from(ctx.getImageData(x,${Math.floor(focused.y + focused.height / 2)},1,1).data))})()`
      )
      const rgb = (pixel: number[]) => 'rgb(' + pixel.slice(0, 3).join(',') + ')'
      expect(
        Math.max(
          colorContrast(rgb(pixels[0]!), rgb(pixels[2]!)),
          colorContrast(rgb(pixels[1]!), rgb(pixels[2]!))
        )
      ).toBeGreaterThanOrEqual(3)
      expect(focused.color).toBe('rgb(255, 255, 255)')
      await key('Enter')
      expect(await actionCount()).toBe('1')
    }
  )
  it.each(sizes)(
    'should remain readable in forced colors without changing %s target geometry',
    async (size) => {
      await ready('?size=' + size)
      const normal = await metrics()
      await browser.session.send('Emulation.setEmulatedMedia', {
        features: [{ name: 'forced-colors', value: 'active' }]
      })
      await ready('?size=' + size)
      const forced = await metrics()
      expect([forced.width, forced.height]).toEqual([normal.width, normal.height])
      expect(contrast(forced)).toBeGreaterThanOrEqual(
        requiredTextContrast(forced.fontSize, forced.fontWeight)
      )
      await key('Tab')
      await key('Tab')
      expect((await metrics()).focused).toBe(true)
    }
  )
  it.each(['disabled', 'loading'])(
    'should preserve inactive native %s state and suppress actual pointer activation',
    async (state) => {
      await ready('?' + state)
      const m = await metrics()
      await pointer('mouseMoved', m)
      await pointer('mousePressed', m)
      await pointer('mouseReleased', m)
      expect((await metrics()).disabled).toBe(state === 'disabled')
      expect(await browser.evaluate(`document.querySelector('output').textContent`)).toBe('0')
      expect((await metrics()).color).toBe('rgb(255, 255, 255)')
      if (state === 'loading') expect((await metrics()).busy).toBe('true')
    }
  )
})

describe('when the primary design foreground shares the native Button root', () => {
  it.each([
    ['secondary', 'rgb(252, 252, 252)'],
    ['dark', 'rgb(255, 255, 255)'],
    ['outline', 'rgb(252, 252, 252)'],
    ['outlineOnLight', 'rgb(22, 21, 24)'],
    ['inverse', 'rgb(37, 33, 42)'],
    ['text', 'rgb(252, 252, 252)']
  ])('should preserve the existing %s foreground', async (variant, color) => {
    await ready('?variant=' + variant)
    expect((await metrics()).color).toBe(color)
  })
  it('should preserve the transparent icon-only foreground instead of applying filled-primary text paint', async () => {
    await ready('?compact')
    const m = await metrics()
    expect(m.color).toBe('rgb(255, 255, 255)')
    expect(m.background).toBe('rgba(0, 0, 0, 0)')
  })
  it.each(['hover', 'pressed', 'focus'])(
    'should retain design-white authored %s state previews',
    async (force) => {
      await ready('?force=' + force)
      const m = await metrics()
      expect(m.color).toBe('rgb(255, 255, 255)')
    }
  )
})

describe('when button fixture rendering cannot finish normally', () => {
  it('should wait for an actual delayed mount rather than sampling a missing target', async () => {
    await ready('?failure=delayed')
    expect((await metrics()).width).toBeGreaterThan(0)
  })
  it('should report missing target readiness within its frame limit', async () => {
    await expect(ready('?failure=missing')).rejects.toThrow(
      'Button fixture did not settle within 120 frames'
    )
  })
  it('should report the actual renderer error instead of waiting for a missing target', async () => {
    expectedRuntimeError = 'Button fixture render rejected'
    await expect(ready('?failure=render')).rejects.toThrow('Button fixture render rejected')
  })
})
