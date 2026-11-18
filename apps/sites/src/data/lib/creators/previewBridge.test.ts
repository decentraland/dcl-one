import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PreviewMessageType } from '@dcl/schemas'
import { createPreviewBridge, type PreviewBridge } from './previewBridge'

function mountIframe() {
  const iframe = document.createElement('iframe')
  iframe.src = 'https://wearable-preview.decentraland.zone/?profile=default'
  document.body.appendChild(iframe)
  const postMessage = vi.fn()
  Object.defineProperty(iframe, 'contentWindow', { value: { postMessage }, configurable: true })
  return { iframe, postMessage }
}

const ORIGIN = 'https://wearable-preview.decentraland.zone'

function ready(iframe: HTMLIFrameElement, origin = ORIGIN) {
  window.dispatchEvent(
    new MessageEvent('message', { data: { type: PreviewMessageType.READY }, source: iframe.contentWindow, origin })
  )
}

let bridge: PreviewBridge | undefined

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
  bridge?.dispose()
  vi.useRealTimers()
  document.body.innerHTML = ''
})

describe('createPreviewBridge', () => {
  it('holds updates until the iframe is ready, then posts the full option set to its origin', () => {
    const { iframe, postMessage } = mountIframe()
    bridge = createPreviewBridge({ iframe })
    bridge.update({ skin: 'aaaaaa' })
    vi.runAllTimers()
    expect(postMessage).not.toHaveBeenCalled()

    ready(iframe)
    expect(postMessage).toHaveBeenCalledWith(
      { type: PreviewMessageType.UPDATE, payload: { options: { skin: 'aaaaaa' } } },
      ORIGIN
    )
    expect(bridge.boots).toBe(1)
  })

  it('coalesces a burst of changes into one post after the debounce', () => {
    const { iframe, postMessage } = mountIframe()
    const onPost = vi.fn()
    bridge = createPreviewBridge({ iframe, onPost })
    ready(iframe)
    bridge.update({ skin: '111111' })
    bridge.update({ skin: '222222' })
    bridge.update({ skin: '333333' })
    expect(postMessage).not.toHaveBeenCalled()
    vi.advanceTimersByTime(300)
    expect(postMessage).toHaveBeenCalledTimes(1)
    expect(postMessage.mock.calls[0][0].payload.options).toEqual({ skin: '333333' })
    expect(onPost).toHaveBeenCalledWith({ skin: '333333' })
  })

  it('re-sends the current set after every boot and ignores messages from anywhere else', () => {
    const { iframe, postMessage } = mountIframe()
    bridge = createPreviewBridge({ iframe })
    bridge.update({ skin: 'aaaaaa' })
    // Another window, and the right window claiming a foreign origin.
    window.dispatchEvent(
      new MessageEvent('message', { data: { type: PreviewMessageType.READY }, source: window, origin: ORIGIN })
    )
    ready(iframe, 'https://evil.example')
    expect(postMessage).not.toHaveBeenCalled()
    ready(iframe)
    ready(iframe)
    expect(postMessage).toHaveBeenCalledTimes(2)
    expect(bridge.boots).toBe(2)
  })

  it('drops an update that says nothing new, since the iframe rebuilds its scene for each one', () => {
    const { iframe, postMessage } = mountIframe()
    bridge = createPreviewBridge({ iframe })
    ready(iframe)
    bridge.update({ skin: 'aaaaaa' })
    vi.advanceTimersByTime(300)
    bridge.update({ skin: 'aaaaaa' })
    vi.advanceTimersByTime(300)
    expect(postMessage).toHaveBeenCalledTimes(1)
    bridge.update({ skin: 'bbbbbb' })
    vi.advanceTimersByTime(300)
    expect(postMessage).toHaveBeenCalledTimes(2)
    // A reloaded scene knows nothing, so a boot re-sends even an unchanged set.
    ready(iframe)
    expect(postMessage).toHaveBeenCalledTimes(3)
  })

  it('stops listening once disposed', () => {
    const { iframe, postMessage } = mountIframe()
    bridge = createPreviewBridge({ iframe })
    bridge.update({ skin: 'aaaaaa' })
    bridge.dispose()
    ready(iframe)
    expect(postMessage).not.toHaveBeenCalled()
  })
})
