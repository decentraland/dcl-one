import path from 'node:path'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { findBrowser, openSession } from '../figma/review/scripts/capture-browser.mjs'

type Measurement = {
  width: number
  color: string
  labelColor: string | null
  iconColor: string | null
  background: string
}
let server: ViteDevServer
let session: Awaited<ReturnType<typeof openSession>>

async function evaluate<T = unknown>(expression: string): Promise<T> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true
  })) as {
    result: { value: T }
    exceptionDetails?: { text: string; exception?: { description?: string } }
  }
  if (result.exceptionDetails)
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text)
  return result.result.value
}

beforeAll(async () => {
  const root = path.resolve(import.meta.dirname, '..')
  server = await createServer({
    configFile: false,
    root,
    appType: 'custom',
    logLevel: 'error',
    resolve: { dedupe: ['react', 'react-dom'] },
    server: { host: '127.0.0.1', port: 0 }
  })
  server.middlewares.use(async (request, response, next) => {
    if (request.url !== '/') return next()
    response.setHeader('Content-Type', 'text/html')
    response.end(
      await server.transformIndexHtml(
        '/',
        '<!doctype html><style>body{padding:20px;background:white}main{display:flex;flex-direction:column;align-items:flex-start;gap:24px}</style><div id="root"></div><script type="module" src="/test/consumer/button-browser.tsx"></script>'
      )
    )
  })
  await server.listen()
  session = await openSession(
    await findBrowser([
      process.env.BROWSER_PATH,
      process.env.CHROME_PATH,
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      '/usr/bin/google-chrome',
      '/usr/bin/chromium',
      '/usr/bin/chromium-browser'
    ])
  )
  await session.send('Page.navigate', { url: server.resolvedUrls!.local[0] })
  await evaluate(
    `(async()=>{for(let n=0;n<100 && document.querySelectorAll('button').length!==10;n++)await new Promise(r=>setTimeout(r,100));await document.fonts.ready})()`
  )
}, 30_000)

afterAll(async () => {
  try {
    await session?.close()
  } finally {
    await server?.close()
  }
})

const measure = (control: string): Promise<Measurement> =>
  evaluate<Measurement>(
    `(()=>{const root=document.querySelector('[data-control="${control}"]'),label=root.querySelector('[data-button-label]'),icon=root.querySelector('svg');return {width:root.getBoundingClientRect().width,color:getComputedStyle(root).color,labelColor:label&&getComputedStyle(label).color,iconColor:icon&&getComputedStyle(icon).color,background:getComputedStyle(root).backgroundColor}})()`
  )

describe('when a public Button enters a native interaction state', () => {
  it.each([390, 1440])(
    'should keep label geometry and foreground through all interaction states at %ipx',
    async (width) => {
      await session.send('Emulation.setDeviceMetricsOverride', {
        width,
        height: 900,
        deviceScaleFactor: 1,
        mobile: false
      })
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 0, y: 0 })
      const base = await measure('base')
      const point = await evaluate<{ x: number; y: number }>(
        `(()=>{const r=document.querySelector('[data-control="base"]').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`
      )
      await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
      const hover = await measure('base')
      await session.send('Input.dispatchMouseEvent', {
        type: 'mousePressed',
        button: 'left',
        clickCount: 1,
        ...point
      })
      const active = await measure('base')
      await session.send('Input.dispatchMouseEvent', {
        type: 'mouseReleased',
        button: 'left',
        clickCount: 1,
        ...point
      })
      const forcedHover = await measure('hover')
      const forcedPressed = await measure('pressed')
      for (const state of [base, hover, active, forcedHover, forcedPressed]) {
        expect(state.width).toBeCloseTo(base.width, 1)
        expect(state.iconColor).toBe(state.labelColor)
        expect(state.color).toBe(state.labelColor)
      }
      expect(hover.labelColor).not.toBe(base.labelColor)
      expect(forcedHover.labelColor).toBe(hover.labelColor)
      expect(forcedPressed.labelColor).toBe(active.labelColor)
      expect(session.runtimeErrors).toEqual([])
    }
  )

  it('should keep an icon-only pressed appearance neutral', async () => {
    const icon = await measure('icon')
    const neutral = await evaluate<string>(
      `(()=>{const probe=document.createElement('span');probe.style.backgroundColor='var(--dcl-bg-hover)';document.body.append(probe);const color=getComputedStyle(probe).backgroundColor;probe.remove();return color})()`
    )
    expect(icon.background).toBe(neutral)
    expect(icon.width).toBeGreaterThanOrEqual(48)
  })
})

describe('when busy and focusable disabled controls use the native component', () => {
  it('should retain label geometry and center every busy indicator within its control', async () => {
    for (const [idle, busy] of [
      ['base', 'loading'],
      ['plain', 'loading-plain']
    ]) {
      const before = await measure(idle!)
      const after = await measure(busy!)
      expect(after.width).toBeCloseTo(before.width, 1)
      const placement = await evaluate<{
        dx: number
        dy: number
        inside: boolean
        mask: string
        color: string
        fill: string
      }>(
        `(()=>{const root=document.querySelector('[data-control="${busy}"]'),spinner=root.querySelector('[data-loading-indicator]');if(!spinner)throw new Error(root.outerHTML);const box=root.getBoundingClientRect(),r=spinner.getBoundingClientRect(),s=getComputedStyle(spinner);return {dx:r.x+r.width/2-(box.x+box.width/2),dy:r.y+r.height/2-(box.y+box.height/2),inside:r.left>=box.left&&r.right<=box.right&&r.top>=box.top&&r.bottom<=box.bottom,mask:s.borderTopStyle,color:getComputedStyle(root).color,fill:s.borderTopColor}})()`
      )
      expect(placement.dx).toBeCloseTo(0, 1)
      expect(placement.dy).toBeCloseTo(0, 1)
      expect(placement.inside).toBe(true)
      expect(placement.mask).toBe('solid')
      expect(placement.fill).toBe(placement.color)
    }
    const doc = (await session.send('DOM.getDocument')) as { root: { nodeId: number } }
    const selected = (await session.send('DOM.querySelector', {
      nodeId: doc.root.nodeId,
      selector: '[data-control="loading"]'
    })) as { nodeId: number }
    const described = (await session.send('DOM.describeNode', { nodeId: selected.nodeId })) as {
      node: { backendNodeId: number }
    }
    const accessibility = (await session.send('Accessibility.getPartialAXTree', {
      backendNodeId: described.node.backendNodeId,
      fetchRelatives: false
    })) as { nodes: { role?: { value: string }; name?: { value: string } }[] }
    expect(accessibility.nodes.find((node) => node.role?.value === 'button')?.name?.value).toBe(
      'A LONGER BUTTON LABEL'
    )
  })

  it('should keep aria-disabled visuals equal to native disabled through pointer states', async () => {
    const native = await evaluate<string>(
      `getComputedStyle(document.querySelector('[data-control="native-disabled"]')).opacity`
    )
    const before = await measure('aria-disabled')
    const point = await evaluate<{ x: number; y: number }>(
      `(()=>{const el=document.querySelector('[data-control="aria-disabled"]');el.focus();const r=el.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`
    )
    await session.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point })
    await session.send('Input.dispatchMouseEvent', {
      type: 'mousePressed',
      button: 'left',
      clickCount: 1,
      ...point
    })
    const after = await measure('aria-disabled')
    await session.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased',
      button: 'left',
      clickCount: 1,
      ...point
    })
    expect(after).toEqual(before)
    const result = await evaluate<{ opacity: string; focused: boolean; disabled: boolean }>(
      `(()=>{const el=document.querySelector('[data-control="aria-disabled"]');return {opacity:getComputedStyle(el).opacity,focused:document.activeElement===el,disabled:el.disabled}})()`
    )
    expect(result.opacity).toBe(native)
    expect(Number(result.opacity)).toBeLessThan(1)
    expect(result.focused).toBe(true)
    expect(result.disabled).toBe(false)
  })
})

it('should preserve control identity and native focus behavior across loading transitions', async () => {
  const result = await evaluate<{
    buttonFocusedBefore: boolean
    buttonMatchesNativeBusyFocus: boolean
    buttonStable: boolean
    noFocusSteal: boolean
    linkFocusedBefore: boolean
    linkRetainsFocus: boolean
    linkStable: boolean
    linkHrefRestored: boolean
  }>(`(async()=>{

    const button=document.querySelector('[data-control="transition"]'),link=document.querySelector('[data-control="transition-link"]');
    const setBusy=async value=>{window.dispatchEvent(new CustomEvent('button-busy',{detail:value}));for(let i=0;i<100 && (button.getAttribute('aria-busy')==='true')!==value;i++)await new Promise(r=>setTimeout(r,10))};
    button.focus();const buttonFocusedBefore=document.activeElement===button;
    await setBusy(true);await new Promise(requestAnimationFrame);const buttonMatchesNativeBusyFocus=document.activeElement===button,buttonStable=button===document.querySelector('[data-control="transition"]');
    const focusOwner=document.createElement('button');focusOwner.textContent='Other action';document.body.append(focusOwner);focusOwner.focus();await setBusy(false);const noFocusSteal=document.activeElement===focusOwner;focusOwner.remove();
    link.focus();const linkFocusedBefore=document.activeElement===link;await setBusy(true);const linkRetainsFocus=document.activeElement===link;await setBusy(false);
    return {buttonFocusedBefore,buttonMatchesNativeBusyFocus,buttonStable,noFocusSteal,linkFocusedBefore,linkRetainsFocus,linkStable:link===document.querySelector('[data-control="transition-link"]'),linkHrefRestored:link.getAttribute('href')==='/save'};
  })()`)
  expect(Object.values(result)).toEqual(Array(8).fill(true))
})

it('should stop the native loading animation when reduced motion is requested', async () => {
  await session.send('Emulation.setEmulatedMedia', {
    features: [{ name: 'prefers-reduced-motion', value: 'reduce' }]
  })
  try {
    expect(
      await evaluate<string>(
        `getComputedStyle(document.querySelector('[data-control="loading"] [data-loading-indicator]')).animationName`
      )
    ).toBe('none')
  } finally {
    await session.send('Emulation.setEmulatedMedia', { features: [] })
  }
})
