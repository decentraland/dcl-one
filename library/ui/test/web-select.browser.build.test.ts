// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { waitForFixture } from './fixture-state.helpers'
import { productionBrowser } from './production-browser.helpers'

const b = productionBrowser('test/consumer/web-select-browser.tsx')

type Frame = { x: number; y: number; width: number; height: number }

async function ready() {
  const errorStart = b.session.runtimeErrors.length
  await b.session.send('Emulation.setDeviceMetricsOverride', {
    width: 1440,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false
  })
  await b.session.navigate(b.origin)
  await waitForFixture(
    'WebSelect fixture',
    () =>
      b.evaluate<boolean>(
        `Boolean(document.querySelector('[data-case="wide"]')?.getBoundingClientRect().width && document.fonts.status === 'loaded')`
      ),
    Boolean,
    () => b.evaluate('new Promise(resolve=>requestAnimationFrame(()=>resolve(null)))'),
    () => b.session.runtimeErrors.slice(errorStart)
  )
}

function frame(name: string) {
  return b.evaluate<Frame>(
    `(() => { const r = document.querySelector('[data-case="${name}"]').parentElement.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } })()`
  )
}

/**
 * The browser draws the select's arrow, so the DOM has no box for it. Across the middle of the
 * field, each run of columns that differ from the field's own fill is ink: the last run is the arrow,
 * the one before it the end of the selected text.
 */
async function inkRuns(box: Frame) {
  const png = (await b.session.send('Page.captureScreenshot', {
    format: 'png',
    clip: { ...box, scale: 1 }
  })) as { data: string }
  return b.evaluate<[number, number][]>(`(async () => {
    const image = new Image(); image.src = 'data:image/png;base64,${png.data}'; await image.decode()
    const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height
    const context = canvas.getContext('2d'); context.drawImage(image, 0, 0)
    const { data, width, height } = context.getImageData(0, 0, image.width, image.height)
    const at = (x, y) => { const k = (y * width + x) * 4; return [data[k], data[k + 1], data[k + 2]] }
    const fill = at(8, Math.round(height / 2))
    const runs = []
    let start = -1
    for (let x = 4; x < width - 4; x++) {
      let ink = false
      for (let y = Math.round(height * 0.3); y < Math.round(height * 0.7) && !ink; y++) {
        const p = at(x, y)
        ink = Math.abs(p[0] - fill[0]) + Math.abs(p[1] - fill[1]) + Math.abs(p[2] - fill[2]) > 60
      }
      if (ink && start < 0) start = x
      if (!ink && start >= 0) { runs.push([start, x - 1]); start = -1 }
    }
    if (start >= 0) runs.push([start, width - 5])
    return runs
  })()`)
}

describe('when a WebSelect shows its selected option beside the native arrow', () => {
  let caseErrorStart = 0
  beforeEach(() => {
    caseErrorStart = b.session.runtimeErrors.length
  })
  afterEach(async () => {
    expect(b.session.runtimeErrors.slice(caseErrorStart)).toEqual([])
    await b.session.send('Emulation.clearDeviceMetricsOverride')
  })

  it.each(['intrinsic', 'digits', 'narrow', 'wide'])(
    'should keep the %s text at least 8px clear of the arrow inside its 48px frame',
    async (name) => {
      await ready()
      const box = await frame(name)
      expect(box.height).toBe(48)
      const runs = await inkRuns(box)
      expect(runs.length).toBeGreaterThanOrEqual(2)
      const arrow = runs[runs.length - 1]!
      const text = runs[runs.length - 2]!
      expect(arrow[0] - text[1] - 1).toBeGreaterThanOrEqual(8)
      // The arrow stays inside the frame's own 16px padding.
      expect(box.width - arrow[1] - 1).toBeGreaterThanOrEqual(16)
    }
  )
})

type LabelBox = { label: Frame; frame: Frame; font: string; color: string; inner: Frame | null }

function labelled(name: string) {
  return b.evaluate<LabelBox>(
    `(() => {
      const control = document.querySelector('[data-case="${name}"]')
      const box = (e) => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
      const label = control.labels[0]
      const style = getComputedStyle(label)
      return {
        label: box(label),
        frame: box(control.parentElement),
        font: style.fontSize + '/' + style.lineHeight + ' ' + style.fontWeight,
        color: style.color,
        inner: label.firstElementChild ? box(label.firstElementChild) : null
      }
    })()`
  )
}

describe('when a WebSelect or WebTextarea shows a visible label', () => {
  afterEach(async () => {
    await b.session.send('Emulation.clearDeviceMetricsOverride')
  })

  it.each(['labelled-select', 'labelled-textarea'])(
    'should draw the %s label as InputWebCreatorTools draws its own, 8px above the frame',
    async (name) => {
      await ready()
      const input = await labelled('labelled-input')
      const field = await labelled(name)
      expect(field.font).toBe(input.font)
      expect(field.color).toBe(input.color)
      expect(field.label.height).toBe(input.label.height)
      expect(field.label.width).toBe(input.label.width)
      expect(field.frame.y - (field.label.y + field.label.height)).toBe(8)
      expect(input.frame.y - (input.label.y + input.label.height)).toBe(8)
    }
  )

  it('should size a label given as an element by that element, not by the label line', async () => {
    await ready()
    const field = await labelled('node-label')
    expect(field.inner).not.toBeNull()
    expect(field.label.height).toBe(field.inner!.height)
    expect(field.label.height).toBeLessThan(18)
  })
})
