import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import { BodyShape, PreviewEmote, PreviewEmoteEventType, PreviewMessageType, PreviewRenderer } from '@dcl/schemas'
import mitt from 'mitt'
import { type AvatarAttributes } from '~/lib/avatar'
import { useAvatarPreview } from '~/store/avatarPreview'

type PreviewProps = Record<string, unknown> & { id: string; onLoad?: () => void }
const previewProps: PreviewProps[] = []
const events = mitt()
const controller = {
  emote: { events, play: vi.fn(), pause: vi.fn(), stop: vi.fn() },
  scene: {},
  physics: { setSpringBonesParams: vi.fn() }
}

vi.mock('~/components/WearablePreview', () => ({
  WearablePreview(props: PreviewProps) {
    previewProps.push(props)
    return <iframe id={props.id} src="https://wearable-preview.decentraland.zone/?x=1" title="preview" />
  },
  createWearablePreviewController: vi.fn(() => controller)
}))

let renderer: PreviewRenderer | undefined = PreviewRenderer.UNITY
vi.mock('~/hooks/usePreviewRenderer', () => ({ usePreviewRenderer: () => renderer }))

const { AvatarPreview } = await import('./AvatarPreview')

const avatar: AvatarAttributes = {
  bodyShape: BodyShape.MALE,
  skin: 'aaaaaa',
  eyes: 'bbbbbb',
  hair: 'cccccc',
  baseWearableUrns: ['urn:a'],
  hasOutfit: true
}
const source = { kind: 'items' as const, items: [] }

function iframe() {
  return document.getElementById('preview') as HTMLIFrameElement
}

function ready() {
  act(() => {
    window.dispatchEvent(
      new MessageEvent('message', {
        data: { type: PreviewMessageType.READY },
        source: iframe().contentWindow,
        // The bridge checks the origin, which a real browser stamps on every message.
        origin: 'https://wearable-preview.decentraland.zone'
      })
    )
  })
}

beforeEach(() => {
  vi.useFakeTimers()
  previewProps.length = 0
  renderer = PreviewRenderer.UNITY
})
afterEach(() => {
  vi.useRealTimers()
  events.all.clear()
})

describe('AvatarPreview', () => {
  it('mounts the iframe once with the avatar snapshot and pushes later changes as UPDATE messages', () => {
    const { rerender } = render(
      <AvatarPreview id="preview" source={source} avatar={avatar} emote={PreviewEmote.WAVE} />
    )
    const postMessage = vi.fn()
    Object.defineProperty(iframe(), 'contentWindow', { value: { postMessage }, configurable: true })
    expect(previewProps[0]).toMatchObject({ skin: 'aaaaaa', urns: ['urn:a'], unity: true, emote: PreviewEmote.WAVE })
    expect(screen.getByTestId('avatar-preview-loading')).toBeInTheDocument()

    ready()
    expect(screen.queryByTestId('avatar-preview-loading')).not.toBeInTheDocument()
    expect(postMessage).toHaveBeenCalledTimes(1)
    expect(postMessage.mock.calls[0][0].payload.options).toMatchObject({
      skin: 'aaaaaa',
      emote: PreviewEmote.WAVE,
      base64s: []
    })

    rerender(
      <AvatarPreview id="preview" source={source} avatar={{ ...avatar, skin: '123456' }} emote={PreviewEmote.WAVE} />
    )
    act(() => {
      vi.advanceTimersByTime(300)
    })
    expect(postMessage).toHaveBeenCalledTimes(2)
    expect(postMessage.mock.calls[1][0].payload.options.skin).toBe('123456')
    // The URL-bound props never change, so ui2 has no reason to reload the iframe.
    expect(previewProps[previewProps.length - 1].skin).toBe('aaaaaa')
  })

  it('waits for the renderer decision before mounting the iframe, then mounts it with that renderer', () => {
    renderer = undefined
    const { rerender } = render(<AvatarPreview id="preview" source={source} avatar={avatar} />)
    expect(screen.getByTestId('avatar-preview-loading')).toBeInTheDocument()
    expect(previewProps).toHaveLength(0)

    renderer = PreviewRenderer.BABYLON
    rerender(<AvatarPreview id="preview" source={source} avatar={avatar} />)
    expect(previewProps[0]).toMatchObject({ unity: false })
  })

  it('reports loads and the controller and mirrors emote playback into the store', () => {
    const onLoad = vi.fn()
    const onController = vi.fn()
    render(<AvatarPreview id="preview" source={source} avatar={avatar} onLoad={onLoad} onController={onController} />)
    expect(onController).toHaveBeenCalledWith(controller)
    act(() => previewProps[0].onLoad?.())
    expect(onLoad).toHaveBeenCalled()

    act(() => events.emit(PreviewEmoteEventType.ANIMATION_PLAY))
    expect(useAvatarPreview.getState().isPlaying).toBe(true)
    act(() => events.emit(PreviewEmoteEventType.ANIMATION_PAUSE))
    expect(useAvatarPreview.getState().isPlaying).toBe(false)
  })

  it('returns the avatar to idle once a one-shot default emote has ended', () => {
    useAvatarPreview.getState().setEmote(PreviewEmote.WAVE)
    render(<AvatarPreview id="preview" source={source} avatar={avatar} emote={PreviewEmote.WAVE} />)
    act(() => events.emit(PreviewEmoteEventType.ANIMATION_PLAY))
    act(() => events.emit(PreviewEmoteEventType.ANIMATION_END))
    expect(useAvatarPreview.getState().isPlaying).toBe(false)
    expect(useAvatarPreview.getState().emote).toBe(PreviewEmote.IDLE)
  })

  it('renders its children as the overlay', () => {
    render(
      <AvatarPreview id="preview" source={source} avatar={avatar}>
        <button type="button">Play</button>
      </AvatarPreview>
    )
    expect(screen.getByTestId('avatar-preview-overlay')).toHaveTextContent('Play')
  })
})
