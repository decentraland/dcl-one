import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PreviewEmote, PreviewEmoteEventType, type IPreviewController } from '@dcl/schemas'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { ItemType, type Item } from '~/lib/items'
import { useAvatarPreview } from '~/store/avatarPreview'
import { PlaybackBar } from './PlaybackBar'

// ui2's scrubber lazy-loads MUI and talks to an iframe; the bar's own controls are what's under test.
vi.mock('~/components/WearablePreview', () => ({
  EmoteControls: () => <div data-testid="ui2-emote-controls" />
}))

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

const emote = (id: string): Item =>
  ({
    id,
    name: id,
    type: ItemType.EMOTE,
    data: { representations: [] },
    contents: {}
  }) as unknown as Item

const dance = emote('dance-item')
const hat = { id: 'hat-item', name: 'Party Hat', tokenId: '7', type: ItemType.WEARABLE } as unknown as Item
const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function makeController({ hasSound = false } = {}) {
  const stop = vi.fn().mockResolvedValue(undefined)
  const disableSound = vi.fn().mockResolvedValue(undefined)
  const enableSound = vi.fn().mockResolvedValue(undefined)
  let soundLoaded = false
  const listeners = new Map<string, Set<() => void>>()
  const events = {
    on: (type: string, fn: () => void) => listeners.set(type, (listeners.get(type) ?? new Set()).add(fn)),
    off: (type: string, fn: () => void) => listeners.get(type)?.delete(fn)
  }
  // The iframe only knows about the emote's audio once it has loaded it, which it announces by playing.
  const loadEmote = () =>
    act(async () => {
      soundLoaded = hasSound
      listeners.get(PreviewEmoteEventType.ANIMATION_PLAY)?.forEach(fn => fn())
    })
  const emote = { stop, disableSound, enableSound, events, hasSound: async () => soundLoaded }
  return { stop, disableSound, enableSound, loadEmote, controller: { emote } as unknown as IPreviewController }
}

/** Mirrors the editor: whichever collection emote is on the avatar becomes the preview's subject. */
function Bar({ controller }: { controller: IPreviewController }) {
  const dressedItemIds = useAvatarPreview(state => state.dressedItemIds)
  const subject = [dance].find(item => dressedItemIds.includes(item.id)) ?? null
  return (
    <PlaybackBar
      previewId="preview"
      controller={controller}
      collectionEmotes={[dance]}
      previewedWearables={[]}
      subjectEmoteId={subject?.id ?? null}
    />
  )
}

describe('PlaybackBar', () => {
  beforeEach(() => {
    useAvatarPreview.getState().clearDressed()
    useAvatarPreview.getState().setEmote(PreviewEmote.IDLE)
  })

  it('plays a collection emote and stops it again, back to the picker', async () => {
    const { stop, controller } = makeController()
    render(<Bar controller={controller} />, { wrapper })

    await userEvent.click(screen.getByTestId('playback-bar-select'))
    await userEvent.click(screen.getByTestId('playback-bar-select-option-dance-item'))
    expect(useAvatarPreview.getState().dressedItemIds).toEqual(['dance-item'])
    expect(screen.getByTestId('ui2-emote-controls')).toBeInTheDocument()

    await userEvent.click(screen.getByTestId('playback-bar-stop'))
    expect(stop).toHaveBeenCalled()
    expect(useAvatarPreview.getState().dressedItemIds).toEqual([])
    expect(screen.getByTestId('playback-bar-select')).toBeInTheDocument()
  })

  it('reports the emote played alongside each wearable on the avatar, as the legacy editor does', async () => {
    const { controller } = makeController()
    render(
      <PlaybackBar
        previewId="preview"
        controller={controller}
        collectionEmotes={[dance]}
        previewedWearables={[hat]}
        subjectEmoteId={null}
      />,
      { wrapper }
    )

    await userEvent.click(screen.getByTestId('playback-bar-select'))
    await userEvent.click(screen.getByTestId('playback-bar-select-option-wave'))

    expect(track).toHaveBeenCalledWith('Play Emote', {
      EMOTE_PLAYED_BASE: true,
      EMOTE_PLAYED_ITEM_ID: null,
      EMOTE_PLAYED_NAME: 'wave',
      PREVIEWED_WEARABLE_ITEM_ID: '7',
      PREVIEWED_WEARABLE_NAME: 'Party Hat'
    })
  })

  it('still reports an emote played on a bare avatar, where the legacy editor reported nothing', async () => {
    const { controller } = makeController()
    render(<Bar controller={controller} />, { wrapper })

    await userEvent.click(screen.getByTestId('playback-bar-select'))
    await userEvent.click(screen.getByTestId('playback-bar-select-option-dance-item'))

    expect(track).toHaveBeenCalledWith('Play Emote', {
      EMOTE_PLAYED_BASE: false,
      EMOTE_PLAYED_ITEM_ID: null,
      EMOTE_PLAYED_NAME: 'dance-item',
      PREVIEWED_WEARABLE_ITEM_ID: null,
      PREVIEWED_WEARABLE_NAME: null
    })
  })

  it('stops a default animation and offers it again', async () => {
    const { stop, controller } = makeController()
    render(<Bar controller={controller} />, { wrapper })

    await userEvent.click(screen.getByTestId('playback-bar-select'))
    await userEvent.click(screen.getByTestId('playback-bar-select-option-wave'))
    expect(useAvatarPreview.getState().emote).toBe(PreviewEmote.WAVE)

    await userEvent.click(screen.getByTestId('playback-bar-stop'))
    expect(stop).toHaveBeenCalled()
    expect(useAvatarPreview.getState().emote).toBe(PreviewEmote.IDLE)
    expect(screen.queryByTestId('playback-bar-stop')).not.toBeInTheDocument()
  })

  it('offers mute once the emote turns out to have audio, and keeps it muted across reloads', async () => {
    const { controller, loadEmote, disableSound, enableSound } = makeController({ hasSound: true })
    render(<Bar controller={controller} />, { wrapper })
    act(() => useAvatarPreview.getState().dress({ id: dance.id, type: ItemType.EMOTE }))
    expect(screen.queryByTestId('playback-bar-sound')).not.toBeInTheDocument()

    await loadEmote()
    await userEvent.click(screen.getByTestId('playback-bar-sound'))
    expect(disableSound).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('playback-bar-sound')).toHaveAttribute('data-muted', 'true')

    await loadEmote()
    expect(disableSound).toHaveBeenCalledTimes(2)

    await userEvent.click(screen.getByTestId('playback-bar-sound'))
    expect(enableSound).toHaveBeenCalled()
    expect(screen.getByTestId('playback-bar-sound')).not.toHaveAttribute('data-muted')
  })

  it('shows no sound toggle for a silent emote', async () => {
    const { controller, loadEmote } = makeController()
    render(<Bar controller={controller} />, { wrapper })
    act(() => useAvatarPreview.getState().dress({ id: dance.id, type: ItemType.EMOTE }))
    await loadEmote()
    expect(screen.queryByTestId('playback-bar-sound')).not.toBeInTheDocument()
  })

  it('keeps working when the preview has no controller yet', async () => {
    render(
      <PlaybackBar
        previewId="preview"
        controller={null}
        collectionEmotes={[dance]}
        previewedWearables={[]}
        subjectEmoteId="dance-item"
      />,
      { wrapper }
    )
    useAvatarPreview.getState().dress({ id: dance.id, type: ItemType.EMOTE })

    await userEvent.click(screen.getByTestId('playback-bar-stop'))
    expect(useAvatarPreview.getState().dressedItemIds).toEqual([])
  })
})
