// One person in a photo. The wearables are the ones they had on when the photo was taken, not their
// current outfit. A guest has no passport and no wearables.

import { Button, Spinner } from '@dcl/one-ui'
import { useEffect, useRef, useState } from 'react'
import { Avatar, DclLogo, MaskIcon, VerifiedBadge, rarityTile } from '../../design'
import { track } from '@explorer-core/lib/analytics'
import { userColor } from '@explorer-core/lib/identity'
import type { GalleryPerson, Wearable } from '@explorer-core/engine/protocol'
import { useProfile } from '../session/profileStore'
import { CategoryIcon } from '../backpack/categoryIcons'
import type { ChatUser } from '../chat/ProfileCardPresentation'
import { openExternal, type OpenExternalUrl } from './reelActions'
import arrowDown from '../../assets/events/arrow-down.png'
import styles from './PhotoDetail.module.css'

/** The link opens once the press animation has played. */
const BUY_DELAY_MS = 300

export function PhotoPersonRow({
  person,
  loadWearables,
  onViewPerson,
  openExternalUrl
}: {
  person: GalleryPerson
  loadWearables: (urns: string[]) => Promise<Wearable[] | null>
  onViewPerson?: (user: ChatUser) => void
  openExternalUrl?: OpenExternalUrl
}): React.JSX.Element {
  const profile = useProfile(person.address)
  const claimed = profile?.hasClaimedName === true
  const color = userColor(person.address, person.name, profile?.hasClaimedName, profile?.nameColor)
  const interactive = person.isGuest !== true
  const [open, setOpen] = useState(false)
  // undefined: not read yet; null: the read failed. Read once, on first open.
  const [wearables, setWearables] = useState<Wearable[] | null | undefined>(undefined)
  const urns = person.wearables

  useEffect(() => {
    if (!open || wearables !== undefined) return
    if (urns == null || urns.length === 0) {
      setWearables([])
      return
    }
    let live = true
    void loadWearables(urns).then((list) => {
      if (live) setWearables(list)
    })
    return () => {
      live = false
    }
  }, [open, wearables, urns, loadWearables])

  const listId = `photo-person-${person.address}`
  return (
    <div className={styles.person}>
      <div className={styles.personCard}>
        <button
          type="button"
          className={styles.personName}
          aria-label={[person.name || person.address, claimed ? 'verified' : null, person.official === true ? 'official' : null].filter(Boolean).join(', ')}
          disabled={!interactive || onViewPerson == null}
          onClick={() => onViewPerson?.({ address: person.address, name: person.name, picture: profile?.picture, claimed, nameColor: profile?.nameColor })}
        >
          <Avatar src={profile?.picture} name={person.name || person.address} color={color} size={40} framed className={styles.personAvatar} />
          <span className={styles.nameLine}>
            <span className={styles.nameText} style={{ color }}>
              {person.name}
            </span>
            {profile != null && !claimed && <span className={styles.nameTag}>#{person.address.slice(-4)}</span>}
            {claimed && <VerifiedBadge size={14} />}
            {person.official === true && (
              <span className={styles.official} aria-label="Official" title="Official">
                <DclLogo size={13} />
              </span>
            )}
          </span>
        </button>
        {interactive && (
          <button
            type="button"
            className={styles.expand}
            aria-label={open ? `Hide ${person.name}'s wearables` : `Show ${person.name}'s wearables`}
            aria-expanded={open}
            aria-controls={listId}
            onClick={() => setOpen((v) => !v)}
          >
            <MaskIcon src={arrowDown} width={14} height={9} className={open ? styles.expandOpen : styles.expandIcon} />
          </button>
        )}
      </div>
      {open && (
        <div id={listId} className={styles.wearables}>
          {wearables === undefined ? (
            <div className={styles.wearablesLoading}>
              <Spinner size={30} variant="chase" />
            </div>
          ) : wearables == null || wearables.length === 0 ? (
            <p className={styles.wearablesEmpty}>No collectibles equipped when the photo was taken</p>
          ) : (
            wearables.map((w) => <PhotoWearable key={w.urn} wearable={w} openExternalUrl={openExternalUrl} />)
          )}
        </div>
      )}
      <span className={styles.personSeparator} aria-hidden="true" />
    </div>
  )
}

function PhotoWearable({ wearable, openExternalUrl }: { wearable: Wearable; openExternalUrl?: OpenExternalUrl }): React.JSX.Element {
  const [failed, setFailed] = useState(false)
  const [pressed, setPressed] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])
  const shopUrl = wearable.shopUrl
  return (
    <div className={styles.wearable} data-rarity={wearable.rarity}>
      <span className={styles.wearableThumb} style={{ backgroundImage: `url(${rarityTile(wearable.rarity)})` }}>
        {wearable.thumbnail != null && !failed && <img className={styles.wearableImg} src={wearable.thumbnail} alt="" onError={() => setFailed(true)} />}
        <span className={styles.wearableFlap} aria-hidden="true">
          <span className={styles.wearableFlapTint} />
          <span className={styles.wearableFlapIcon}>
            <CategoryIcon category={wearable.category} size={11} />
          </span>
        </span>
      </span>
      <span className={styles.wearableName} title={wearable.name}>
        {wearable.name}
      </span>
      {shopUrl != null && (
        <Button appearance="hud"
          variant="primary"
          size="bare"
          className={`${styles.buy} ${pressed ? styles.buyPressed : ''}`.trim()}
          aria-label={`Buy ${wearable.name}`}
          onAnimationEnd={() => setPressed(false)}
          onClick={() => {
            setPressed(true)
            clearTimeout(timer.current)
            timer.current = setTimeout(() => {
              openExternal(shopUrl, openExternalUrl)
              track('photo_to_marketplace')
            }, BUY_DELAY_MS)
          }}
        >
          BUY
        </Button>
      )}
    </div>
  )
}
