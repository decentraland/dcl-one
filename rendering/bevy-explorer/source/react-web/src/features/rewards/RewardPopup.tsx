// Reward popup, as unity-explorer's RewardPanel: a reward_in_progress notification from the poll
// opens it on its own (RewardPanelPlugin.cs), next to the usual new-notification toast. It shows
// the item on its rarity tile with rays in the rarity colour, and COLLECT closes it. Rewards that
// arrive together open one after another. Imperative like the toasts: queueRewardPopups() from
// the session; the popup renders through the shared popup layer.

import type { AppNotification } from '../../engine/protocol'
import { Button, openPopup, rarityTile, type Rarity } from '../../design'
import { nativeProfileImage } from '../../lib/baseDomain'
import { CategoryIcon } from '../backpack/categoryIcons'
import styles from './RewardPopup.module.css'

export const REWARD_TYPE = 'reward_in_progress'

const RARITIES: readonly Rarity[] = ['base', 'common', 'uncommon', 'rare', 'epic', 'legendary', 'mythic', 'unique', 'exotic']

export interface Reward {
  name?: string
  image?: string
  rarity: Rarity
  category?: string
}

function str(m: Record<string, unknown>, key: string): string | undefined {
  const v = m[key]
  return typeof v === 'string' && v !== '' ? v : undefined
}

/** The reward a notification announces, from the fields unity's RewardInProgressNotification
 *  reads (tokenName, tokenImage, tokenRarity, tokenCategory); null for any other type. */
export function rewardOf(n: AppNotification): Reward | null {
  if (n.type !== REWARD_TYPE) return null
  const m = n.metadata ?? {}
  const rarity = str(m, 'tokenRarity')?.toLowerCase()
  return {
    name: str(m, 'tokenName'),
    image: nativeProfileImage(str(m, 'tokenImage')),
    rarity: RARITIES.find((r) => r === rarity) ?? 'base',
    category: str(m, 'tokenCategory')
  }
}

let queue: Reward[] = []
let closeCurrent: (() => void) | null = null

function showNext(): void {
  closeCurrent = null
  const next = queue.shift()
  if (next == null) return
  closeCurrent = openPopup((close) => <RewardPopup reward={next} onCollect={close} />, {
    // Like unity, only COLLECT (or the central Cancel) dismisses it, not a stray click.
    backdropClickCloses: false,
    onClose: showNext
  })
}

/** Open a popup for each reward in the list, in the order given (oldest first). */
export function queueRewardPopups(list: AppNotification[]): void {
  const rewards = list.map(rewardOf).filter((r): r is Reward => r != null)
  if (rewards.length === 0) return
  queue = [...queue, ...rewards]
  if (closeCurrent == null) showNext()
}

/** Close the reward on screen and drop the ones waiting (logout). */
export function clearRewardPopups(): void {
  queue = []
  const close = closeCurrent
  closeCurrent = null
  close?.()
}

export function RewardPopup({ reward, onCollect }: { reward: Reward; onCollect: () => void }): React.JSX.Element {
  return (
    <div
      className={styles.root}
      data-rarity={reward.rarity}
      role="dialog"
      aria-modal="true"
      aria-labelledby="reward-popup-title"
      onClick={(e) => e.stopPropagation()}
    >
      <span className={styles.rays} aria-hidden="true" />
      <div className={styles.content}>
        <span className={styles.header}>Quest reward</span>
        <h2 id="reward-popup-title" className={styles.title}>New Item Unlocked!</h2>
        <div className={styles.item} style={{ backgroundImage: `url(${rarityTile(reward.rarity)})` }}>
          {reward.image != null && <img className={styles.thumb} src={reward.image} alt="" />}
          {reward.category != null && (
            <span className={styles.flap} aria-hidden="true">
              <span className={styles.flapTint} />
              <span className={styles.flapIcon}>
                <CategoryIcon category={reward.category} size={20} />
              </span>
            </span>
          )}
        </div>
        {reward.name != null && <p className={styles.name}>{reward.name}</p>}
        <p className={styles.note}>Processing may take a few minutes, but you&apos;ll be notified as soon as it&apos;s in your Backpack!</p>
        <Button size="lg" onClick={onCollect}>
          COLLECT
        </Button>
      </div>
    </div>
  )
}
