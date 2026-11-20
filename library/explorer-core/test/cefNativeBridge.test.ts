import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

describe("the native bridge's clipboard", () => {
  // jsdom has no clipboard: each test defines the webview's, and it goes with the test
  const browserWrite = vi.fn(async (_text: string) => undefined)
  let emitted: unknown[]
  let hear: (id: string, payload: unknown) => void
  // the bridge listens on the document for the page's life: a test takes its listeners away
  let listening: [string, EventListenerOrEventListenerObject][]
  // each test is one load of the page: the bridge sets the clipboard up once per document
  let installCefNativeBridge: () => void

  const asked = { to: 'engine', kind: 'engineClipboard' }
  const copied = (text: string): object => ({ to: 'engine', kind: 'copyToClipboard', text })

  const installNative = (): void => {
    const listeners = new Map<string, (payload: unknown) => void>()
    hear = (id, payload) => listeners.get(id)?.(payload)
    ;(window as unknown as { cef: unknown }).cef = {
      emit: (payload: unknown) => emitted.push(payload),
      listen: (id: string, cb: (payload: unknown) => void) => listeners.set(id, cb)
    }
    vi.stubGlobal('BroadcastChannel', class { postMessage() {} })
    installCefNativeBridge()
  }
  /** The engine's window is on Wayland: it answers the page's request. */
  const installOnWayland = (): void => {
    installNative()
    expect(emitted.splice(0)).toEqual([asked])
    hear('engineClipboard', '')
  }

  beforeEach(async () => {
    vi.resetModules()
    ;({ installCefNativeBridge } = await import('../lib/cefNativeBridge'))
    emitted = []
    listening = []
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: browserWrite }, configurable: true })
    const listen = document.addEventListener.bind(document)
    vi.spyOn(document, 'addEventListener').mockImplementation((type: string, listener: EventListenerOrEventListenerObject) => {
      listening.push([type, listener])
      listen(type, listener)
    })
  })
  afterEach(() => {
    for (const [type, listener] of listening) document.removeEventListener(type, listener)
    delete (window as unknown as { cef?: unknown }).cef
    Reflect.deleteProperty(navigator, 'clipboard')
    browserWrite.mockClear()
    document.body.innerHTML = ''
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('sends what the page copies to the engine: a written text, and a copied or cut selection', async () => {
    installOnWayland()

    await expect(navigator.clipboard.writeText('0xabc')).resolves.toBeUndefined()
    expect(browserWrite).not.toHaveBeenCalled()

    document.body.innerHTML = '<p>picked words</p>'
    document.dispatchEvent(new Event('copy'))
    document.getSelection()!.selectAllChildren(document.body.firstElementChild!)
    document.dispatchEvent(new Event('copy'))
    document.dispatchEvent(new Event('cut'))

    expect(emitted).toEqual([copied('0xabc'), copied('picked words'), copied('picked words')])
  })

  it("never asks the engine for the clipboard: it stops the webview's paste and types the text the engine sends", () => {
    installOnWayland()
    const execCommand = vi.fn(() => true)
    document.execCommand = execCommand

    expect(document.dispatchEvent(new Event('paste', { cancelable: true }))).toBe(false)
    expect(emitted).toEqual([])
    expect(execCommand).not.toHaveBeenCalled()

    hear('paste', 'from the session')
    expect(execCommand).toHaveBeenCalledWith('insertText', false, 'from the session')
  })

  it('types the text into the field that had the focus at the paste key, and drops it when the focus moved', () => {
    installOnWayland()
    const execCommand = vi.fn(() => true)
    document.execCommand = execCommand
    document.body.innerHTML = '<input id="chat"><input id="search">'
    const chat = document.getElementById('chat')!
    const search = document.getElementById('search')!

    chat.focus()
    document.dispatchEvent(new Event('paste', { cancelable: true }))
    search.focus()
    hear('paste', 'for the chat')
    expect(execCommand).not.toHaveBeenCalled()

    // the dropped paste left no field behind: a text with no paste of the webview goes to the focus
    hear('paste', 'for the focus')
    expect(execCommand).toHaveBeenCalledWith('insertText', false, 'for the focus')

    chat.focus()
    document.dispatchEvent(new Event('paste', { cancelable: true }))
    hear('paste', 'for the chat')
    expect(execCommand).toHaveBeenLastCalledWith('insertText', false, 'for the chat')
    expect(execCommand).toHaveBeenCalledTimes(2)
  })

  it('forgets the field of a paste after the time the engine gives a text', () => {
    installOnWayland()
    const execCommand = vi.fn(() => true)
    document.execCommand = execCommand
    document.body.innerHTML = '<input id="chat"><input id="search">'
    const now = vi.spyOn(Date, 'now').mockReturnValue(1_000)

    // a paste with an empty clipboard: the engine sends no text for it
    document.getElementById('chat')!.focus()
    document.dispatchEvent(new Event('paste', { cancelable: true }))

    now.mockReturnValue(1_000 + 2_001)
    document.getElementById('search')!.focus()
    hear('paste', 'for the search')
    expect(execCommand).toHaveBeenCalledWith('insertText', false, 'for the search')
  })

  it('leaves a paste that a handler of the page took for itself', () => {
    installOnWayland()
    const execCommand = vi.fn(() => true)
    document.execCommand = execCommand
    document.body.innerHTML = '<input id="chat">'
    const chat = document.getElementById('chat')!
    chat.addEventListener('paste', (e) => e.preventDefault())

    chat.focus()
    chat.dispatchEvent(new Event('paste', { bubbles: true, cancelable: true }))
    hear('paste', 'from the session')
    expect(execCommand).not.toHaveBeenCalled()
  })

  it('sets the clipboard up once in a document, however often the page mounts and the engine answers', () => {
    installOnWayland()
    installCefNativeBridge()
    expect(emitted.splice(0)).toEqual([asked])
    hear('engineClipboard', '')
    hear('engineClipboard', '')
    const execCommand = vi.fn(() => true)
    document.execCommand = execCommand

    document.body.innerHTML = '<p>picked words</p>'
    document.getSelection()!.selectAllChildren(document.body.firstElementChild!)
    document.dispatchEvent(new Event('copy'))
    hear('paste', 'from the session')

    expect(emitted).toEqual([copied('picked words')])
    expect(execCommand).toHaveBeenCalledTimes(1)
    expect(listening.map(([type]) => type)).toEqual(['copy', 'cut', 'paste'])
  })

  it.each([
    ['until the engine answers (a window on X11, macOS, Windows)', installNative, [asked]],
    ['where window.cef is absent (the web)', () => installCefNativeBridge(), []]
  ])("keeps the webview's clipboard %s", async (_where, install, requests) => {
    install()

    await navigator.clipboard.writeText('0xabc')
    expect(browserWrite).toHaveBeenCalledWith('0xabc')
    expect(document.dispatchEvent(new Event('paste', { cancelable: true }))).toBe(true)
    expect(listening).toEqual([])
    expect(emitted).toEqual(requests)
  })
})
