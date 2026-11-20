import { readFileSync } from 'node:fs'
import { type Page } from 'playwright-core'
import { type App } from './app'
import { waitForEnabled } from './dom'
import { FIXTURE } from './files'

/** Metrics the fake renderer reports for any model, with values no fixture row uses so they can be traced. */
export const PREVIEW_METRICS = { triangles: 1234, materials: 2, textures: 2, meshes: 1, bodies: 1, entities: 1 }

/** What the renderer measures for its model-less boot scene: Babylon keeps one empty mesh, with no geometry. */
export const BOOT_SCENE_METRICS = { triangles: 0, materials: 0, textures: 0, meshes: 1, bodies: 1, entities: 1 }

export type PreviewOptions = {
  /**
   * Holds every model's LOAD until the page posts `{ type: 'e2e_release' }` to the iframe. Each held load is
   * reported as a `load: held` answer (see app.previewAnswers): release only after one, or the message can reach
   * the iframe before this page has loaded and be lost.
   */
  holdLoads?: boolean
}

const pages = new Map<boolean, string>()

/**
 * Stands in for the wearable-preview app, which can't render headless. It speaks the same postMessage protocol
 * (`@dcl/schemas` PreviewMessageType): READY on boot, LOAD after boot and after every UPDATE (plus a play event for
 * emotes), and an answer to every controller request, with the fixture thumbnail as the screenshot and sound
 * reported when the item ships an audio file. Metrics read the boot scene until a model sent in an UPDATE has loaded.
 * Each frame also reports its numbered steps (`frame` answers: `<kind>-<id>:<n>:<step>`, steps `booted`, `ready`, `update`,
 * `model-loaded`, and `gone` when it unloads) so a spec that times out can tell where a preview stopped (see `waitForProcessedAction`).
 */
export function previewPageHtml({ holdLoads = false }: PreviewOptions = {}): string {
  const cached = pages.get(holdLoads)
  if (cached) return cached
  const html = `<!doctype html><title>preview</title><script>
    var held = ${holdLoads};
    // The draft processor's iframe is the orthographic one without background or rotation.
    var KIND = /projection=orthographic/.test(location.search) && /disableAutoRotate=true/.test(location.search) &&
      /disableBackground=true/.test(location.search) ? 'processor' : 'preview';
    var FRAME = KIND + '-' + Math.random().toString(36).slice(2, 6);
    function report(method, result) {
      fetch('/__e2e/answer?method=' + method + '&result=' + encodeURIComponent(result), { keepalive: true }).catch(function () {});
    }
    var steps = 0;
    // Numbered, because the reports travel as separate requests and can land out of order.
    function step(name) { report('frame', FRAME + ':' + steps++ + ':' + name); }
    step('booted');
    // A frame that unmounted can't be the one stalling.
    window.addEventListener('pagehide', function () { step('gone'); });
    var pending = 0;
    var SCREENSHOT = 'data:image/png;base64,${readFileSync(FIXTURE.thumbnail).toString('base64')}';
    var METRICS = ${JSON.stringify(PREVIEW_METRICS)};
    var BOOT_SCENE_METRICS = ${JSON.stringify(BOOT_SCENE_METRICS)};
    // Whether a model sent in an UPDATE has loaded; until then metrics read the model-less boot scene.
    var hasModel = false;
    var RESULTS = {
      'scene.getScreenshot': SCREENSHOT,
      'emote.getLength': 2,
      'emote.isPlaying': false
    };
    // Whether the item last sent in an UPDATE ships an audio file, as the real renderer would find out.
    var hasSound = false;
    function send(type, payload) { parent.postMessage({ type: type, payload: payload }, '*'); }
    function loaded() { send('load', { renderer: 'babylon' }); }
    function modelLoaded() {
      hasModel = true;
      step('model-loaded');
      loaded();
    }
    function answer(key) {
      if (key === 'emote.hasSound') return hasSound;
      if (key === 'scene.getMetrics') return hasModel ? METRICS : BOOT_SCENE_METRICS;
      return RESULTS[key];
    }
    window.addEventListener('message', function (event) {
      var message = event.data || {};
      if (message.type === 'update') {
        step('update');
        var options = (message.payload && message.payload.options) || {};
        var text = JSON.stringify(message.payload, function (key, value) { return value instanceof Blob ? undefined : value; }) || '';
        // Saved items arrive as base64 definitions, decoded the way wearable-preview does.
        (options.base64s || []).forEach(function (encoded) { try { text += atob(encoded); } catch (e) {} });
        hasSound = /\\.(mp3|ogg|wav)"/i.test(text);
        if (held) {
          pending++;
          report('load', 'held');
        } else modelLoaded();
        // An emote autoplays once loaded; the editor asks about its sound when it starts.
        if (/emoteDataADR74/.test(text)) send('emote_event', { type: 'animation_play', payload: {} });
      }
      if (message.type === 'e2e_release') {
        held = false;
        for (; pending > 0; pending--) modelLoaded();
      }
      if (message.type === 'controller_request') {
        var request = message.payload || {};
        var key = request.namespace + '.' + request.method;
        var result = answer(key);
        send('controller_response', { id: request.id, ok: true, result: result });
        // Lets a spec wait for the answer itself instead of a fixed delay (see app.previewAnswers).
        if (key === 'emote.hasSound') report(key, String(result));
      }
    });
    // The parent may attach its listener after this frame boots, so READY is repeated for a moment.
    var sent = 0;
    var boot = setInterval(function () {
      send('ready');
      if (sent === 0) step('ready');
      if (sent === 0) loaded();
      if (++sent > 10) clearInterval(boot);
    }, 100);
  </script>`
  pages.set(holdLoads, html)
  return html
}

const BOOT_STEPS = ['booted', 'ready', 'update']

/** Where a frame stopped: a boot step it never reached, or a model it got in its last UPDATE and never loaded. */
function stalledBefore(steps: Array<string | undefined>): string | undefined {
  if (steps.includes('gone')) return undefined
  const lastUpdate = steps.lastIndexOf('update')
  if (lastUpdate < 0) {
    // A later step proves the earlier ones, even when their reports were lost.
    const reached = Math.max(...Array.from(steps, step => BOOT_STEPS.indexOf(step ?? '')))
    return BOOT_STEPS[reached + 1]
  }
  // One iframe processes a batch one draft after another, so only what followed the latest UPDATE counts.
  return steps.slice(lastUpdate).includes('model-loaded') ? undefined : 'model-loaded'
}

/** Each preview frame's steps, in the order it took them, naming where it stopped. */
export function describePreviewFrames(answers: App['previewAnswers']): string {
  const frames = new Map<string, string[]>()
  for (const { method, result } of answers) {
    if (method !== 'frame') continue
    const [id, index, step] = result.split(':')
    const steps = frames.get(id) ?? []
    steps[Number(index)] = step
    frames.set(id, steps)
  }
  if (frames.size === 0) return 'No preview frame booted.'
  return [...frames]
    .map(([id, steps]) => {
      const stalled = stalledBefore(steps)
      // A gap is a report still in flight or lost, not a step the frame skipped.
      const taken = Array.from(steps, step => step ?? '?').join(' → ')
      return `${id}: ${taken}${stalled ? ` (stopped before ${stalled})` : ''}`
    })
    .join('\n')
}

/**
 * Waits for an action the draft processing gates (Save, Finish). When it stays disabled, the error lists what every
 * preview frame on the page got through, so a timeout says whether the processing stalled and where.
 */
export async function waitForProcessedAction(
  app: Pick<App, 'page' | 'previewAnswers'> & Partial<Pick<App, 'events' | 'consoleErrors'>>,
  id: string,
  timeout?: number
): Promise<void> {
  try {
    await waitForEnabled(app.page, id, timeout)
  } catch (error) {
    const processed = ((await app.events?.()) ?? [])
      .filter(event => event.event === 'Item preview processed')
      .map(({ props }) =>
        JSON.stringify({
          item_type: props.item_type,
          outcome: props.outcome,
          duration_ms: props.duration_ms,
          attempts: props.attempts
        })
      )
    // The form fields a draft needs before it can be saved, as the page shows them now.
    const form = await app.page
      .$$eval('[data-testid="add-items-modal"] [data-testid]', els =>
        els
          .filter(el => /^(item-|draft-|add-items-finish)/.test(el.getAttribute('data-testid') ?? ''))
          .map(el => {
            const input = el as HTMLInputElement
            const value = 'value' in input && typeof input.value === 'string' ? `=${JSON.stringify(input.value)}` : ''
            const state = [...el.attributes]
              .filter(attr => attr.name.startsWith('data-') && attr.name !== 'data-testid')
              .map(attr => `${attr.name}=${attr.value}`)
              .join(',')
            return `${el.getAttribute('data-testid')}${value}${state ? `[${state}]` : ''}${input.disabled ? '(disabled)' : ''}`
          })
          .slice(0, 40)
      )
      .catch(() => ['add-items modal not found'])
    throw new Error(
      [
        `[data-testid="${id}"] stayed disabled.`,
        `Preview frames:\n${describePreviewFrames(app.previewAnswers)}`,
        `Item preview processed: ${processed.join(' | ') || 'none'}`,
        `Form: ${form.join(' ') || 'none'}`,
        `Console errors:\n${app.consoleErrors?.slice(-8).join('\n') || 'none'}`
      ].join('\n'),
      { cause: error }
    )
  }
}

/**
 * From now on, while a draft-processing preview is loading its page, another preview on the page keeps announcing
 * READY: the worst moment for it, since the processor's own page is not listening yet.
 */
export async function announceReadyFromAnotherPreview(page: Page): Promise<void> {
  await page.evaluate(() => {
    new MutationObserver(() => {
      const frame = document.querySelector<HTMLIFrameElement>('[data-testid="draft-processor"] iframe')
      if (!frame || frame.dataset.announced) return
      frame.dataset.announced = 'true'
      const origin = new URL(frame.src).origin
      const announce = () => window.dispatchEvent(new MessageEvent('message', { data: { type: 'ready' }, origin }))
      // Repeated until the page is in, so one lands after the app starts listening whenever that is.
      const repeating = window.setInterval(announce, 20)
      frame.addEventListener('load', () => window.clearInterval(repeating), { once: true })
      announce()
    }).observe(document.body, { childList: true, subtree: true })
  })
}
