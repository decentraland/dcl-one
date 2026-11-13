// The lobby's profile menu, dropped from the profile in the top bar: who is signed in (picture, name,
// wallet with copy), then Profile, Settings, Report a Bug, Sign Out and Exit, and the legal links.
// A click outside or Escape closes it.

import { useEffect, useRef, useState } from 'react'
import bugIcon from '../../assets/lobby/bug-icon.png'
import checkmark from '../../assets/lobby/checkmark-icn-thin.png'
import copyIcon from '../../assets/lobby/icon-copy.png'
import defaultProfile from '../../assets/lobby/default-profile-thumbnail.png'
import leaveIcon from '../../assets/lobby/leave-icon.png'
import playersIcon from '../../assets/lobby/players-icn.png'
import settingsIcon from '../../assets/lobby/settings-icn.png'
import signOutIcon from '../../assets/lobby/sign-out-icn.png'
import verifiedIcon from '../../assets/lobby/icon-verified.png'
import { MaskIcon } from '../../design'
import { siteUrl } from '@hud-host'
import styles from './LobbyProfileMenu.module.css'

const COPIED_MS = 1000

/** The wallet as the menu shows it: its first five characters and its last five. */
export const menuAddress = (a: string): string => (a.length > 12 ? `${a.slice(0, 5)}...${a.slice(-5)}` : a)

export function LobbyProfileMenu({
  name,
  address,
  picture,
  verified,
  nameColor,
  withClose,
  onClose,
  onProfile,
  onSettings,
  onReportBug,
  onSignOut,
  onExit
}: {
  name: string
  address: string | null
  picture: string | null
  verified: boolean
  nameColor: string
  /** The top bar shows its Close button, so the menu sits clear of it. */
  withClose: boolean
  onClose: () => void
  onProfile: () => void
  onSettings: () => void
  onReportBug: (() => void) | null
  onSignOut: () => void
  onExit: (() => void) | null
}): React.JSX.Element {
  const [copied, setCopied] = useState(false)
  const first = useRef<HTMLButtonElement>(null)
  useEffect(() => {
    first.current?.focus()
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [onClose])
  useEffect(() => {
    if (!copied) return
    const id = setTimeout(() => setCopied(false), COPIED_MS)
    return () => clearTimeout(id)
  }, [copied])
  const item = (label: string, icon: string, run: () => void, ref?: React.Ref<HTMLButtonElement>): React.JSX.Element => (
    <button
      ref={ref}
      type="button"
      role="menuitem"
      className={styles.item}
      onClick={() => {
        onClose()
        run()
      }}
    >
      <MaskIcon src={icon} size={22} />
      <span className={styles.itemLabel}>{label}</span>
    </button>
  )
  return (
    <div className={styles.menu}>
      <div className={styles.backdrop} onClick={onClose} />
      <div className={`${styles.panel} ${withClose ? styles.withClose : ''}`.trim()} role="menu" aria-label="Profile menu">
        <div className={styles.identity}>
          <span className={styles.picture} style={{ backgroundImage: `url("${picture || defaultProfile}")` }} />
          <div className={styles.texts}>
            <div className={styles.nameRow}>
              <span className={styles.name} style={{ color: nameColor }}>
                {name || 'Guest'}
              </span>
              {verified && <img className={styles.verified} src={verifiedIcon} alt="Verified" width={16} height={16} />}
            </div>
            {address && (
              <>
                <span className={styles.caption}>User ID</span>
                <span className={styles.addressRow}>
                  <span className={styles.address}>{menuAddress(address)}</span>
                  <button
                    type="button"
                    className={styles.copy}
                    aria-label="Copy wallet address"
                    onClick={() => {
                      navigator.clipboard?.writeText(address).then(
                        () => setCopied(true),
                        () => undefined
                      )
                    }}
                  >
                    <MaskIcon src={copyIcon} size={16} />
                  </button>
                  <span className={`${styles.copied} ${copied ? styles.copiedShown : ''}`.trim()} role="status">
                    <img src={checkmark} alt="" width={12} height={12} />
                    Wallet copied!
                  </span>
                </span>
              </>
            )}
          </div>
        </div>
        <div className={`${styles.separator} ${styles.separatorIdentity}`} />
        {item('Profile', playersIcon, onProfile, first)}
        {item('Settings', settingsIcon, onSettings)}
        {onReportBug && item('Report a Bug', bugIcon, onReportBug)}
        <div className={styles.separator} />
        {item('Sign Out', signOutIcon, onSignOut)}
        {onExit && item('Exit', leaveIcon, onExit)}
        <div className={styles.footer}>
          <a className={styles.link} href={siteUrl('/terms')} target="_blank" rel="noopener noreferrer">
            Terms of Service
          </a>
          <a className={styles.link} href={siteUrl('/privacy')} target="_blank" rel="noopener noreferrer">
            Privacy Policy
          </a>
        </div>
      </div>
    </div>
  )
}
