// TODO(figma-ds): replace with the Figma-generated profile header from library/ui/components.
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import {
  GeneralCheck,
  GeneralCopy,
  GeneralVerified,
  GeneralWallet
} from '../../../../foundations/icons'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import styles from './ProfileHeader.module.css'

type ProfileHeaderProps = {
  address: string
  /** The address as the app shortens it for display (`0x1234…abcd`); the copy button copies `address`. */
  addressLabel: string
  name: string
  /** The profile's name color, also the avatar fallback background. */
  nameColor: string
  /** A claimed NAME shows the verified mark; an unclaimed one the `#1234` suffix. */
  hasClaimedName: boolean
  avatarUrl?: string
  /** Buttons on the right: the owner's calls to action, or a member's. */
  actions?: ReactNode
}

type CopyState = 'idle' | 'copied' | 'failed'

/**
 * The live region stays mounted and only its text changes, so screen readers announce the result.
 * A denied or missing clipboard (an insecure page, a refused permission) says so instead of failing
 * silently.
 */
function CopyAddress({ address }: { address: string }) {
  const [state, setState] = useState<CopyState>('idle')
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])
  const show = (next: CopyState) => {
    setState(next)
    clearTimeout(timer.current)
    timer.current = setTimeout(() => setState('idle'), 2000)
  }
  const copy = () => {
    const clipboard = typeof navigator === 'undefined' ? undefined : navigator.clipboard
    if (!clipboard) return show('failed')
    clipboard.writeText(address).then(
      () => show('copied'),
      () => show('failed')
    )
  }
  return (
    <span className={styles.copyWrap}>
      <button type="button" className={styles.copy} aria-label="Copy address" onClick={copy}>
        <GeneralCopy size={16} aria-hidden="true" />
      </button>
      <span className={state === 'idle' ? styles.copiedHidden : styles.copied} role="status">
        {state === 'copied' ? (
          <>
            <GeneralCheck size={14} aria-hidden="true" /> Address copied!
          </>
        ) : state === 'failed' ? (
          "Couldn't copy the address"
        ) : null}
      </span>
    </span>
  )
}

/** Avatar, name and address on the left, actions on the right; stacked on narrow screens. */
function ProfileHeader({
  address,
  addressLabel,
  name,
  nameColor,
  hasClaimedName,
  avatarUrl,
  actions
}: ProfileHeaderProps) {
  const style = { '--name-color': nameColor } as CSSProperties
  return (
    <header className={styles.root} style={style}>
      <div className={styles.identity}>
        <ProfileAvatar
          imageUrl={avatarUrl}
          name={name}
          color={nameColor}
          className={styles.avatar}
        />
        <div className={styles.nameBlock}>
          <div className={styles.nameRow}>
            <h1 className={styles.name}>{name}</h1>
            {hasClaimedName ? (
              <span className={styles.verified} role="img" aria-label="Verified" title="Verified">
                <GeneralVerified size={20} aria-hidden="true" />
              </span>
            ) : (
              <span className={styles.discriminator}>#{address.slice(-4)}</span>
            )}
          </div>
          <div className={styles.addressRow}>
            <GeneralWallet size={18} aria-hidden="true" />
            <span className={styles.address}>{addressLabel}</span>
            <CopyAddress address={address} />
          </div>
        </div>
      </div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </header>
  )
}

export { ProfileHeader }
export type { ProfileHeaderProps }
