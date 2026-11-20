// NATIVE via CEF (feature react-hud-cef): the HUD runs in a CEF offscreen webview composited by
// the engine, and window.cef.{emit,listen} (injected by cef_offscreen's render process at
// document start) is the transport. This shim rides the page's Envelopes over it — the app
// itself stays transport-agnostic.
//
// No-op when window.cef is absent (web).

import { bridgeChannelName } from '../engine/protocol'
import { answerHudRequest, answerHudWait, type HudRequest } from './hudAutomation'

interface CefApi {
  emit: (payload: unknown) => void
  listen: (id: string, cb: (payload: unknown) => void) => void
}

function cefApi(): CefApi | null {
  const cef = (window as Window & { cef?: CefApi }).cef
  if (!cef?.emit || !cef?.listen) return null
  return cef
}

let clipboardGoesToEngine = false
// The engine drops a paste whose text comes later than this after the key
// (rendering/bevy-explorer/source/src/wayland_clipboard.rs).
const PASTE_FIELD_MS = 2000

// A window on Wayland (rendering/bevy-explorer/source/src/wayland_clipboard.rs): this webview
// draws offscreen, and there a client with no focused surface can neither set nor read the
// session's clipboard. The engine can. Every copy goes to it. A paste is the engine's to start,
// from the paste keys of the window: no message of this page makes it read the clipboard. It
// sends the text as `paste`, typed here into the field that had the focus at the key, and the
// webview's own paste of the same key is stopped. Once per document.
function routeClipboardToEngine(cef: CefApi): void {
  if (clipboardGoesToEngine) return
  clipboardGoesToEngine = true
  const copy = (text: string): void => cef.emit({ to: 'engine', kind: 'copyToClipboard', text })
  const clipboard: Clipboard | undefined = navigator.clipboard
  if (clipboard != null) {
    clipboard.writeText = (text) => {
      copy(text)
      return Promise.resolve()
    }
  }
  for (const type of ['copy', 'cut']) {
    document.addEventListener(type, () => {
      const text = document.getSelection()?.toString() ?? ''
      if (text !== '') copy(text)
    })
  }
  // The field with the focus at the paste key. The engine's text comes later: it goes to that
  // field only, and is dropped when the focus has moved, or when a handler of the page took the
  // paste for itself.
  let pasteField: { field: Element | null; taken: boolean; at: number } | null = null
  document.addEventListener('paste', (e) => {
    pasteField = { field: document.activeElement, taken: e.defaultPrevented, at: Date.now() }
    e.preventDefault()
  })
  cef.listen('paste', (text) => {
    const asked = pasteField
    pasteField = null
    // No field on record (the text came before the webview's own paste, or that paste is an old
    // one): the text goes to the focus, as the key did.
    const known = asked != null && Date.now() - asked.at <= PASTE_FIELD_MS
    if (known && (asked.taken || asked.field !== document.activeElement)) return
    document.execCommand('insertText', false, String(text))
  })
}

export function installCefNativeBridge(): void {
  const cef = cefApi()
  if (cef == null) return

  const ch = new BroadcastChannel(bridgeChannelName())
  // page -> engine: only scene-bound Envelopes cross the boundary
  ch.onmessage = (e) => {
    const env = e.data as { to?: string } | undefined
    if (env?.to === 'scene') cef.emit(env)
  }
  // engine -> page: the native relay emits full Envelopes as JSON strings
  cef.listen('bridge', (payload) => {
    try {
      ch.postMessage(typeof payload === 'string' ? JSON.parse(payload) : payload)
    } catch {
      /* malformed envelope; drop */
    }
  })
  // (HUD focus — including text focus — now flows through the bridge scene as a 'uiFocus'
  // message on every platform; see useEngineSession. The only engine-addressed messages are the
  // clipboard and the console command below, and the stored values further down.)
  // The engine of a window on Wayland answers this, once per load of the page: on X11, macOS and
  // Windows the webview keeps its own clipboard.
  cef.listen('engineClipboard', () => routeClipboardToEngine(cef))
  cef.emit({ to: 'engine', kind: 'engineClipboard' })
  // Engine fps for the perf overlay (see useFps). HUD geometry does NOT come through here:
  // --ui-scale and the engine cutout rects are keyed off the scene's canvas report on every
  // platform (see lib/uiCanvasStore.ts).
  cef.listen('engineFps', (v) => {
    ;(window as Window & { __nativeEngineFps?: number }).__nativeEngineFps = Number(v)
  })
  // Engine-side text focus (scene textinput / engine text box): keys forward to this page
  // unconditionally, so the systemAction dispatcher needs the same don't-treat-keys-as-shortcuts
  // signal boot.js provides on web (see push_text_focus in src/react_hud_cef.rs).
  cef.listen('engineTextFocus', (v) => {
    ;(window as Window & { __engineTextFocus?: boolean }).__engineTextFocus = v === 'true'
  })

  // `window.engine_console_command(line)`: the promise-returning console RPC web.rs exports on web,
  // relayed by src/react_hud_cef.rs (resolves with the command output, rejects with its error).
  const pending = new Map<number, { resolve: (output: string) => void; reject: (e: Error) => void }>()
  let nextId = 1
  cef.listen('consoleReply', (payload) => {
    try {
      const r = JSON.parse(String(payload)) as { id: number; ok: boolean; output: string }
      const call = pending.get(r.id)
      if (call == null) return
      pending.delete(r.id)
      if (r.ok) call.resolve(r.output)
      else call.reject(new Error(r.output))
    } catch {
      /* malformed reply; drop */
    }
  })
  ;(window as Window & { engine_console_command?: (line: string) => Promise<string> }).engine_console_command = (line) =>
    new Promise((resolve, reject) => {
      const id = nextId++
      pending.set(id, { resolve, reject })
      cef.emit({ to: 'engine', kind: 'consoleCommand', id, line })
    })

  // The engine's /hud_* test commands (src/react_hud_cef/automation.rs): it asks which control a
  // selector names and whether a user could click it, then clicks and types through CEF itself.
  // A wait is answered when its state holds, or with an error when its time is up.
  cef.listen('hudAutomation', (payload) => {
    let request: HudRequest
    try {
      request = JSON.parse(String(payload)) as HudRequest
    } catch {
      return /* malformed request; the engine times it out */
    }
    // JSON.parse('null') and JSON.parse('7') succeed: not a request either
    if (!request || typeof request !== 'object' || Array.isArray(request)) return
    const reply = (fields: object): void => cef.emit({ to: 'engine', kind: 'hudAutomationReply', id: request.id, ...fields })
    if (request.op === 'wait') void answerHudWait(request).then(reply)
    else reply({ result: answerHudRequest(request) })
  })
}

export interface CefDownload {
  ok: boolean
  /** The folder it was saved to. */
  dir?: string
}

const downloads = new Map<string, (outcome: CefDownload) => void>()
let hearsDownloads = false

/** How the native download of `url` ended (the engine's CEF download handler reports each one), or
 *  not ok when nothing is heard within `timeoutMs`. Call it before starting the download. Null when
 *  window.cef is absent. */
export function cefDownloadOutcome(url: string, timeoutMs: number): Promise<CefDownload> | null {
  const cef = cefApi()
  if (cef == null) return null
  if (!hearsDownloads) {
    hearsDownloads = true
    cef.listen('download', (payload) => {
      try {
        const r = (typeof payload === 'string' ? JSON.parse(payload) : payload) as { url?: string; ok?: boolean; dir?: string | null }
        if (typeof r.url === 'string') downloads.get(r.url)?.({ ok: r.ok === true, dir: typeof r.dir === 'string' ? r.dir : undefined })
      } catch {
        /* malformed report; the wait times out */
      }
    })
  }
  return new Promise((resolve) => {
    const settle = (outcome: CefDownload): void => {
      clearTimeout(timer)
      downloads.delete(url)
      resolve(outcome)
    }
    const timer = setTimeout(() => settle({ ok: false }), timeoutMs)
    downloads.set(url, settle)
  })
}

export function cefOpenDownloads(): void {
  cefApi()?.emit({ to: 'engine', kind: 'openDownloads' })
}

// The HUD's stored values (lib/prefs). Natively the engine keeps them: CEF's profile is per client
// and deleted when it exits. Null when window.cef is absent.
export function cefPrefs(): { load: () => Promise<Record<string, string>>; set: (key: string, value: string) => void } | null {
  const cef = cefApi()
  if (cef == null) return null
  return {
    load: () =>
      new Promise((resolve) => {
        cef.listen('prefs', (payload) => {
          try {
            resolve(JSON.parse(String(payload)) as Record<string, string>)
          } catch {
            resolve({})
          }
        })
        cef.emit({ to: 'engine', kind: 'prefsLoad' })
      }),
    set: (key, value) => cef.emit({ to: 'engine', kind: 'prefSet', key, value })
  }
}
