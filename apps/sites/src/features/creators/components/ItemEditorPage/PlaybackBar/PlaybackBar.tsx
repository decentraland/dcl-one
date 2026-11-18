import { useEffect, useMemo, useRef, useState } from 'react'
import { PreviewEmote, PreviewEmoteEventType } from '@dcl/schemas'
import { BookmarkIcon as CollectionIcon, StopIcon, VolumeOffIcon, VolumeUpIcon } from '@/features/creators/components/Icons'
import { EmoteControls, type WearablePreviewController } from '@/features/creators/components/WearablePreview'
import { Select } from '@/features/creators/components/Select'
import { Tooltip } from '@/features/creators/components/Tooltip'
import { useMediaQuery } from '@/features/creators/hooks/useMediaQuery'
import { useTranslation } from '@/data/lib/creators/intl'
import { track } from '@/data/lib/creators/analytics'
import { ItemType, type Item } from '@/data/lib/creators/items'
import { useAvatarPreview } from '@/features/creators/store/avatarPreview'
import { media } from '@/data/lib/creators/theme'
import styles from './PlaybackBar.module.css'

type Props = {
  previewId: string
  controller: WearablePreviewController | null
  /** The collection's emotes, offered next to the default animations. */
  collectionEmotes: Item[]
  /** The wearables currently on the avatar — reported alongside the emote, as the legacy editor does. */
  previewedWearables: Item[]
  /** When an emote item is the subject, ui2's scrubber controls replace the play/stop bar. */
  subjectEmoteId: string | null
  testId?: string
}

// Idle is the resting pose, not an emote to pick: "nothing playing" is the empty state of the picker.
const DEFAULT_EMOTES = Object.values(PreviewEmote).filter(
  (value): value is PreviewEmote => typeof value === 'string' && value !== PreviewEmote.IDLE
)

/**
 * Whether the subject emote has audio, and its mute toggle. ui2's own sound button asks once, on mount,
 * before the iframe has loaded the emote, so it never shows; this asks again on every play instead.
 */
function useEmoteSound(controller: WearablePreviewController | null, subjectEmoteId: string | null) {
  const [hasSound, setHasSound] = useState(false)
  const [muted, setMuted] = useState(false)
  const mutedRef = useRef(muted)
  mutedRef.current = muted

  useEffect(() => {
    if (!controller || !subjectEmoteId) return
    let cancelled = false
    // Each load of the emote autoplays it, and Babylon comes back from a reload unmuted.
    const sync = () => {
      controller.emote
        .hasSound()
        .then(result => !cancelled && setHasSound(result))
        .catch(() => undefined)
      if (mutedRef.current) void controller.emote.disableSound().catch(() => undefined)
    }
    sync()
    controller.emote.events.on(PreviewEmoteEventType.ANIMATION_PLAY, sync)
    return () => {
      cancelled = true
      controller.emote.events.off(PreviewEmoteEventType.ANIMATION_PLAY, sync)
      setHasSound(false)
    }
  }, [controller, subjectEmoteId])

  function toggle() {
    const next = !muted
    setMuted(next)
    void (next ? controller?.emote.disableSound() : controller?.emote.enableSound())?.catch(() => undefined)
  }

  return { hasSound, muted, toggle }
}

/**
 * Picks and plays an emote on the avatar, a default one or one of the collection's. Stop returns the
 * avatar to its idle pose, and is the only way out of an emote preview on a phone, where the sidebar's
 * undress button is not there.
 */
export function PlaybackBar({
  previewId,
  controller,
  collectionEmotes,
  previewedWearables,
  subjectEmoteId,
  testId = 'playback-bar'
}: Props) {
  const { t } = useTranslation()
  const isMobile = useMediaQuery(media.maxWidth('mobile'))
  const emote = useAvatarPreview(state => state.emote)
  const dressedItemIds = useAvatarPreview(state => state.dressedItemIds)
  const setEmote = useAvatarPreview(state => state.setEmote)
  const dress = useAvatarPreview(state => state.dress)
  const undress = useAvatarPreview(state => state.undress)
  const sound = useEmoteSound(controller, subjectEmoteId)

  const dressedEmote = useMemo(
    () => collectionEmotes.find(item => dressedItemIds.includes(item.id)) ?? null,
    [collectionEmotes, dressedItemIds]
  )
  const options = useMemo(
    () => [
      ...collectionEmotes.map(item => ({
        value: item.id,
        label: item.name,
        trailing: (
          <Tooltip content={t('item_editor.playback.from_collection')} asChild testId={`${testId}-from-collection`}>
            <span
              className={styles.collectionBadge}
              data-icon-badge
              tabIndex={0}
              aria-label={t('item_editor.playback.from_collection')}
            >
              <CollectionIcon />
            </span>
          </Tooltip>
        )
      })),
      ...DEFAULT_EMOTES.map((value, index) => ({
        value: value,
        label: t(`item_editor.playback.emote.${value}`),
        // The first default emote opens the second group.
        dividerBefore: index === 0 && collectionEmotes.length > 0
      }))
    ],
    [collectionEmotes, t, testId]
  )

  function select(value: string) {
    const collectionEmote = collectionEmotes.find(item => item.id === value)
    // Legacy builder prop names, so the event lines up with the same one from the old item editor,
    // which emits one event per wearable on the avatar. With a bare avatar legacy emits nothing at
    // all; here that case is one event with no wearable, so a pick is never invisible.
    const emoteProps = {
      EMOTE_PLAYED_BASE: !collectionEmote,
      EMOTE_PLAYED_ITEM_ID: collectionEmote?.tokenId ?? null,
      EMOTE_PLAYED_NAME: collectionEmote ? collectionEmote.name : value
    }
    if (previewedWearables.length === 0) {
      track('Play Emote', { ...emoteProps, PREVIEWED_WEARABLE_ITEM_ID: null, PREVIEWED_WEARABLE_NAME: null })
    }
    for (const wearable of previewedWearables) {
      track('Play Emote', {
        ...emoteProps,
        PREVIEWED_WEARABLE_ITEM_ID: wearable.tokenId ?? null,
        PREVIEWED_WEARABLE_NAME: wearable.name
      })
    }
    if (collectionEmote) {
      dress({ id: collectionEmote.id, type: ItemType.EMOTE })
      return
    }
    if (dressedEmote) undress(dressedEmote.id)
    setEmote(value as PreviewEmote)
  }

  function stop() {
    if (dressedEmote) undress(dressedEmote.id)
    setEmote(PreviewEmote.IDLE)
    void controller?.emote.stop().catch(() => undefined)
  }

  const stopButton = (
    <Tooltip content={t('item_editor.playback.stop')} asChild testId={`${testId}-stop-tooltip`}>
      <button
        className={styles.control}
        type="button"
        aria-label={t('item_editor.playback.stop')}
        data-testid={`${testId}-stop`}
        onClick={stop}
      >
        <StopIcon fontSize="small" />
      </button>
    </Tooltip>
  )

  const soundLabel = t(sound.muted ? 'item_editor.playback.unmute' : 'item_editor.playback.mute')
  const soundButton = sound.hasSound && (
    <Tooltip content={soundLabel} asChild testId={`${testId}-sound-tooltip`}>
      <button
        className={styles.control}
        type="button"
        aria-label={soundLabel}
        aria-pressed={sound.muted}
        data-muted={sound.muted || undefined}
        data-testid={`${testId}-sound`}
        onClick={sound.toggle}
      >
        {sound.muted ? <VolumeOffIcon fontSize="small" /> : <VolumeUpIcon fontSize="small" />}
      </button>
    </Tooltip>
  )

  if (subjectEmoteId) {
    return (
      <div className={styles.emoteControlsWrap} data-testid={`${testId}-emote-controls`}>
        {/* The frame counter is what gives way when the row has to fit a phone. */}
        <EmoteControls key={subjectEmoteId} previewId={previewId} hideFrameInput={isMobile} hideSoundButton />
        {soundButton}
        {stopButton}
      </div>
    )
  }

  const isEmoteActive = emote !== PreviewEmote.IDLE || dressedEmote !== null

  return (
    <div className={styles.wrap} data-testid={testId} data-playing={isEmoteActive || undefined}>
      <Select
        value={dressedEmote?.id ?? (isEmoteActive ? emote : null)}
        options={options}
        placeholder={t('item_editor.playback.placeholder')}
        ariaLabel={t('item_editor.playback.select')}
        tone="dark"
        testId={`${testId}-select`}
        onChange={select}
      />
      {isEmoteActive && (
        <button className={styles.stopButton} type="button" data-testid={`${testId}-stop`} onClick={stop}>
          <StopIcon fontSize="small" />
          {t('item_editor.playback.stop')}
        </button>
      )}
    </div>
  )
}
