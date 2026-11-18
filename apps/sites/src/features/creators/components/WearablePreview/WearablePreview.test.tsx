// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BodyShape, PreviewEmoteEventType, PreviewMessageType, type WearableWithBlobs } from '@dcl/schemas'
import { EmoteControls, TranslationControls, WearablePreview, createWearablePreviewController } from '.'

const ORIGIN = 'https://wearable-preview.decentraland.zone'

const frame = () => document.getElementById('preview') as HTMLIFrameElement

/** What the page posted to the preview iframe, in order. */
function watchFrame() {
  const posted = vi.spyOn(frame().contentWindow!, 'postMessage').mockImplementation(() => undefined)
  return () => posted.mock.calls.map(([message]) => message as { type: string; payload: Record<string, unknown> })
}

/** A message arriving from the preview iframe. */
function fromFrame(type: string, payload: unknown = {}, source: MessageEventSource | null = frame().contentWindow) {
  act(() => {
    window.dispatchEvent(new MessageEvent('message', { data: { type, payload }, origin: ORIGIN, source }))
  })
}

afterEach(() => vi.useRealTimers())

describe('WearablePreview', () => {
  it('mounts an iframe under the given id with the options baked into its URL', () => {
    render(<WearablePreview id="preview" bodyShape={BodyShape.FEMALE} skin="aabbcc" urns={['urn:a', 'urn:b']} unity />)

    const url = new URL(frame().src)
    expect(url.origin).toBe(ORIGIN)
    expect(url.searchParams.get('skin')).toBe('aabbcc')
    expect(url.searchParams.get('bodyShape')).toBe(BodyShape.FEMALE)
    expect(url.searchParams.getAll('urn')).toEqual(['urn:a', 'urn:b'])
    expect(url.searchParams.get('unity')).toBe('true')
  })

  it('sends a model the URL cannot carry once the preview says it is ready', () => {
    const blob = { id: 'item' } as WearableWithBlobs
    const onUpdate = vi.fn()
    render(<WearablePreview id="preview" blob={blob} onUpdate={onUpdate} />)
    const posted = watchFrame()

    fromFrame(PreviewMessageType.READY)

    expect(posted()).toEqual([{ type: PreviewMessageType.UPDATE, payload: { options: { blob } } }])
    expect(onUpdate).toHaveBeenCalledTimes(1)
  })

  it('does not repeat to a ready preview what its URL already said', () => {
    render(<WearablePreview id="preview" skin="aabbcc" />)
    const posted = watchFrame()

    fromFrame(PreviewMessageType.READY)

    expect(posted()).toEqual([])
  })

  it('pushes changed options to the mounted preview instead of reloading it', () => {
    vi.useFakeTimers()
    const { rerender } = render(<WearablePreview id="preview" skin="aabbcc" />)
    const src = frame().src
    const posted = watchFrame()
    fromFrame(PreviewMessageType.READY)

    rerender(<WearablePreview id="preview" skin="ddeeff" />)
    act(() => vi.runAllTimers())

    expect(frame().src).toBe(src)
    expect(posted()).toEqual([{ type: PreviewMessageType.UPDATE, payload: { options: { skin: 'ddeeff' } } }])
  })

  it('reports when the preview loaded and when it failed', () => {
    const onLoad = vi.fn()
    const onError = vi.fn()
    render(<WearablePreview id="preview" onLoad={onLoad} onError={onError} />)

    fromFrame(PreviewMessageType.LOAD, { renderer: 'unity' })
    fromFrame(PreviewMessageType.ERROR, { message: 'no model' })

    expect(onLoad).toHaveBeenCalledWith('unity')
    expect(onError).toHaveBeenCalledWith(new Error('no model'))
  })

  it('ignores the messages of another preview on the page', () => {
    const blob = { id: 'item' } as WearableWithBlobs
    const onLoad = vi.fn()
    render(<WearablePreview id="preview" blob={blob} onLoad={onLoad} />)
    const posted = watchFrame()

    fromFrame(PreviewMessageType.READY, {}, window)
    fromFrame(PreviewMessageType.LOAD, {}, window)

    expect(posted()).toEqual([])
    expect(onLoad).not.toHaveBeenCalled()
  })
})

describe('createWearablePreviewController', () => {
  it('asks the preview and answers with what it responds', async () => {
    render(<WearablePreview id="preview" />)
    const posted = watchFrame()
    const controller = createWearablePreviewController('preview')

    const length = controller.emote.getLength()
    const [request] = posted()
    expect(request).toMatchObject({
      type: PreviewMessageType.CONTROLLER_REQUEST,
      payload: { namespace: 'emote', method: 'getLength', params: [] }
    })
    fromFrame(PreviewMessageType.CONTROLLER_RESPONSE, { id: request.payload.id, ok: true, result: 2.5 })

    await expect(length).resolves.toBe(2.5)
  })

  it('rejects a request the preview refuses', async () => {
    render(<WearablePreview id="preview" />)
    const posted = watchFrame()
    const controller = createWearablePreviewController('preview')

    const screenshot = controller.scene.getScreenshot(512, 512)
    fromFrame(PreviewMessageType.CONTROLLER_RESPONSE, { id: posted()[0].payload.id, ok: false, error: 'not loaded' })

    await expect(screenshot).rejects.toThrow('not loaded')
  })

  it('hands the emote events of its preview to whoever listens', () => {
    render(<WearablePreview id="preview" />)
    const controller = createWearablePreviewController('preview')
    const onPlay = vi.fn()
    controller.emote.events.on(PreviewEmoteEventType.ANIMATION_PLAY, onPlay)

    fromFrame(PreviewMessageType.EMOTE_EVENT, { type: PreviewEmoteEventType.ANIMATION_PLAY, payload: {} })

    expect(onPlay).toHaveBeenCalledTimes(1)
  })

  it('needs a mounted preview', () => {
    expect(() => createWearablePreviewController('missing')).toThrow()
  })
})

describe('preview controls', () => {
  /** Answers every request of the controls the way a loaded preview playing a two-second emote with sound would. */
  function answering(posted: ReturnType<typeof watchFrame>) {
    const results: Record<string, unknown> = { getLength: 2, hasSound: true }
    let answered = 0
    return () => {
      const requests = posted().filter(message => message.type === PreviewMessageType.CONTROLLER_REQUEST)
      for (const { payload } of requests.slice(answered)) {
        fromFrame(PreviewMessageType.CONTROLLER_RESPONSE, { id: payload.id, ok: true, result: results[String(payload.method)] })
      }
      answered = requests.length
      return requests.map(({ payload }) => payload.method)
    }
  }

  it('plays and pauses the emote, following what the preview reports', async () => {
    render(
      <>
        <WearablePreview id="preview" />
        <EmoteControls previewId="preview" />
      </>
    )
    const flush = answering(watchFrame())
    fromFrame(PreviewMessageType.LOAD)

    await userEvent.click(screen.getByRole('button', { name: 'Play' }))
    expect(flush()).toContain('play')
    fromFrame(PreviewMessageType.EMOTE_EVENT, { type: PreviewEmoteEventType.ANIMATION_PLAY, payload: {} })
    await act(async () => void flush())

    await userEvent.click(await screen.findByRole('button', { name: 'Pause' }))
    expect(flush()).toContain('pause')
  })

  it('offers to mute an emote that has sound, unless told to hide it', async () => {
    const { rerender } = render(
      <>
        <WearablePreview id="preview" />
        <EmoteControls previewId="preview" />
      </>
    )
    const flush = answering(watchFrame())
    fromFrame(PreviewMessageType.LOAD)
    await act(async () => void flush())

    await userEvent.click(await screen.findByRole('button', { name: 'Mute' }))
    expect(flush()).toContain('disableSound')
    expect(screen.getByRole('button', { name: 'Unmute' })).toBeInTheDocument()

    rerender(
      <>
        <WearablePreview id="preview" />
        <EmoteControls previewId="preview" hideSoundButton />
      </>
    )
    expect(screen.queryByRole('button', { name: 'Unmute' })).not.toBeInTheDocument()
  })

  it('moves the camera up when the slider goes up', () => {
    render(
      <>
        <WearablePreview id="preview" />
        <TranslationControls previewId="preview" vertical verticalPosition="left" />
      </>
    )
    const posted = watchFrame()

    fireEvent.change(screen.getByRole('slider', { name: 'Move the camera up or down' }), { target: { value: '1.5' } })

    expect(posted()[0].payload).toMatchObject({ namespace: 'scene', method: 'panCamera', params: [{ y: -1.5 }] })
  })
})
