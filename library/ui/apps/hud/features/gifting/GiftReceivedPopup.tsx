// The gift-received popup: a `transfer_received` notification, clicked in the
// panel or its toast, opens it. It shows who sent the gift and the item (read from the notification's `tokenUri`) on
// its rarity tile, with rays behind. GO TO BACKPACK opens the Backpack on the item's tab, Emotes for an emote; CLOSE,
// or a click outside, only closes it.

import { useEffect, useState } from 'react'
import type { AppNotification } from '@explorer-core/engine/protocol'
import { loadGiftItem, type GiftableKind, type GiftItem } from '@explorer-core/features/gifting/giftItem'
import { knownUserColor, useProfile } from '../session/profileStore'
import { Button, openPopup } from '../../design'
import { nativeProfileImage } from '@hud-host'
import { ItemTile, RaysPopup } from '../rewards/RewardPopup'
import styles from './GiftReceivedPopup.module.css'

export const GIFT_TYPE = 'transfer_received'
export const GIFT_ON_ITS_WAY = "It's on its way to your Backpack."

const UNKNOWN_ITEM_NAME = 'Unknown Item'

export interface GiftNotice {
  senderAddress: string
  tokenUri?: string
}

/** The gift a notification announces; null for any other type, or one without a sender. */
export function giftOf(n: AppNotification): GiftNotice | null {
  if (n.type !== GIFT_TYPE) return null
  const m = n.metadata ?? {}
  const sender = typeof m.senderAddress === 'string' ? m.senderAddress : ''
  if (!/^0x[0-9a-fA-F]{40}$/.test(sender)) return null
  return { senderAddress: sender, ...(typeof m.tokenUri === 'string' && m.tokenUri ? { tokenUri: m.tokenUri } : {}) }
}

let openBackpack: ((tab: 'wearables' | 'emotes') => void) | null = null

/** How GO TO BACKPACK opens the Backpack; the app sets it once. */
export function setGiftBackpackOpener(fn: ((tab: 'wearables' | 'emotes') => void) | null): void {
  openBackpack = fn
}

/** Opens the popup for a gift notification. */
export function openGiftReceived(gift: GiftNotice): void {
  openPopup((close) => (
    <GiftReceivedPopup
      gift={gift}
      onClose={close}
      onOpenBackpack={(kind) => {
        close()
        openBackpack?.(kind === 'emote' ? 'emotes' : 'wearables')
      }}
    />
  ))
}

type Load = { state: 'loading' } | { state: 'done'; item: GiftItem | null }

export function GiftReceivedPopup({
  gift,
  onClose,
  onOpenBackpack,
  load = loadGiftItem
}: {
  gift: GiftNotice
  onClose: () => void
  onOpenBackpack: (kind: GiftableKind) => void
  load?: (tokenUri: string | undefined) => Promise<GiftItem | null>
}): React.JSX.Element {
  const [item, setItem] = useState<Load>({ state: 'loading' })
  const sender = useProfile(gift.senderAddress)

  useEffect(() => {
    let live = true
    load(gift.tokenUri)
      .catch(() => null)
      .then((found) => {
        if (live) setItem({ state: 'done', item: found })
      })
    return () => {
      live = false
    }
  }, [gift.tokenUri, load])

  const loaded = item.state === 'done' ? item.item : null
  const name = item.state === 'loading' ? 'Loading...' : loaded?.name || UNKNOWN_ITEM_NAME
  const kind: GiftableKind = loaded?.kind ?? 'wearable'
  const rarity = loaded?.rarity ?? 'base'

  return (
    <RaysPopup rarity={rarity} labelledBy="gift-received-title">
      <span className={styles.subtitle}>ON ITS WAY TO YOUR BACKPACK</span>
      <h2 id="gift-received-title" className={styles.title}>
        FROM{' '}
        {sender != null ? (
          <span style={{ color: knownUserColor(gift.senderAddress, sender.name) }}>{sender.name}</span>
        ) : (
          <span>{`${gift.senderAddress.slice(0, 6)}…${gift.senderAddress.slice(-4)}`}</span>
        )}
      </h2>
      <ItemTile rarity={rarity} image={loaded?.imageUrl ? nativeProfileImage(loaded.imageUrl) : undefined} category={loaded?.category} />
      <p className={styles.name} aria-busy={item.state === 'loading'}>
        {name}
      </p>
      <div className={styles.actions}>
        <Button variant="secondary" size="lg" onClick={onClose}>
          CLOSE
        </Button>
        <Button size="lg" onClick={() => onOpenBackpack(kind)}>
          GO TO BACKPACK
        </Button>
      </div>
    </RaysPopup>
  )
}
