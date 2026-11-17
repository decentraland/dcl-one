// The lobby header's profile pill and the menu it opens: the account, Profile, Settings, Report a
// Bug, Sign Out, Exit, and the legal links.

import { useEffect, useRef, useState } from 'react'
import { siteUrl } from '@hud-host'
import { registerCancelLayer } from '@explorer-core/lib/cancelLayers'
import { bugReportUrl } from '@explorer-core/lib/bugReport'
import { shortAddr } from '@explorer-core/lib/identity'
import { Avatar, VerifiedBadge, showConfirm } from '../../design'
import { knownUserColor } from '../session/profileStore'
import styles from './Lobby.module.css'

const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' } as const

function Glyph({ children }: { children: React.ReactNode }): React.JSX.Element {
  return (
    <svg viewBox="0 0 24 24" width="21" height="21" aria-hidden="true" {...stroke}>
      {children}
    </svg>
  )
}

const GLYPHS = {
  profile: (
    <Glyph>
      <circle cx="12" cy="8" r="3.6" />
      <path d="M4.5 20c.6-3.6 3.6-5.6 7.5-5.6s6.900 2 7.500 5.600z" />
    </Glyph>
  ),
  settings: (
    <Glyph>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 3.200l2 .4.800 2.100 2.200-.5 1.500 1.500-.5 2.200 2.100.800.400 2-.4 2-2.100.800.500 2.200-1.500 1.500-2.200-.5-.8 2.100-2 .400-2-.4-.8-2.100-2.200.500-1.500-1.500.500-2.200-2.100-.8-.4-2 .400-2 2.100-.8-.5-2.200 1.500-1.500 2.200.500.800-2.100z" />
    </Glyph>
  ),
  bug: (
    <Glyph>
      <rect x="8" y="8" width="8" height="11" rx="4" />
      <path d="M9.500 8a2.500 2.500 0 0 1 5 0M4 13h4m8 0h4M5 7.500 8.300 10M19 7.500 15.700 10M5 19.500l3.300-2.500M19 19.500 15.700 17M12 11v8" />
    </Glyph>
  ),
  signOut: (
    <Glyph>
      <path d="M12 3v8" />
      <path d="M6.600 6.800a7 7 0 1 0 10.800 0" />
    </Glyph>
  ),
  exit: (
    <Glyph>
      <path d="M13 4H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h6z" />
      <path d="M16 12h5m0 0-2-2m2 2-2 2" stroke="rgb(255, 45, 85)" />
    </Glyph>
  )
}

function Item({ glyph, label, onClick }: { glyph: keyof typeof GLYPHS; label: string; onClick: () => void }): React.JSX.Element {
  return (
    <button type="button" role="menuitem" className={styles.menuItem} onClick={onClick}>
      {GLYPHS[glyph]}
      {label}
    </button>
  )
}

export function LobbyProfile({
  name,
  picture,
  address,
  claimed = false,
  onViewProfile,
  onSettings,
  onSignOut,
  onExit
}: {
  name: string
  picture?: string
  address?: string
  /** The name is a claimed one: the pill shows the verified badge after it. */
  claimed?: boolean
  onViewProfile: () => void
  onSettings: () => void
  /** The startup lobby signs out from here; in world the lobby passes none: there the bar's Me panel has the Sign Out row, for a sign-in on the web the player made. */
  onSignOut?: () => void
  /** In world: leave the lobby. */
  onExit?: () => void
}): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const onDown = (e: MouseEvent): void => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    // Escape closes the menu before the page under it
    const release = registerCancelLayer(() => setOpen(false))
    window.addEventListener('mousedown', onDown)
    return () => {
      window.removeEventListener('mousedown', onDown)
      release()
    }
  }, [open])

  const copy = (): void => {
    if (!address) return
    navigator.clipboard?.writeText(address).then(
      () => {
        setCopied(true)
        setTimeout(() => setCopied(false), 1200)
      },
      () => {}
    )
  }
  const pick = (act: () => void) => (): void => {
    setOpen(false)
    act()
  }
  // Sign Out asks first, as the Me panel's row does in world.
  const askSignOut = (signOut: () => void) => (): void => {
    void showConfirm({ title: 'Sign out?', body: 'You return to the start screen.', cancelLabel: 'Cancel', confirmLabel: 'Sign Out' }).then((confirmed) => {
      if (confirmed) signOut()
    })
  }

  const color = knownUserColor(address ?? '', name)
  return (
    <div className={styles.profile} ref={ref}>
      <button type="button" className={`${styles.pill} ${styles.profilePill} ${open ? styles.pillOn : ''}`.trim()} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {/* signed in and connected: the picture carries the connection dot */}
        <Avatar src={picture} name={name} color={color} size={40} status="online" dotPosition="top" />
        <span className={styles.profileName} style={{ '--profile-name': color } as React.CSSProperties}>
          <span className={styles.profileNameText}>{name}</span>
          {claimed && (
            <span className={styles.profileVerified} aria-hidden="true">
              <VerifiedBadge size={14} />
            </span>
          )}
        </span>
      </button>

      {open && (
        <div className={styles.menu} role="menu" aria-label="Profile">
          <div className={styles.menuHead}>
            <Avatar src={picture} name={name} color={color} size={72} />
            <div className={styles.menuWho}>
              <div className={styles.menuName} style={{ color }}>
                {name}
              </div>
              {address && (
                <>
                  <div className={styles.menuLabel}>User ID</div>
                  <button type="button" className={styles.menuAddress} onClick={copy} title="Copy address">
                    {shortAddr(address)}
                    <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true" {...stroke}>
                      <rect x="4" y="8" width="12" height="12" rx="2" />
                      <path d="M8 5h10a2 2 0 0 1 2 2v9" />
                    </svg>
                    {copied && <span className={styles.menuCopied}>Copied</span>}
                  </button>
                </>
              )}
            </div>
          </div>
          <div className={styles.menuGroup}>
            <Item glyph="profile" label="Profile" onClick={pick(onViewProfile)} />
            <Item glyph="settings" label="Settings" onClick={pick(onSettings)} />
            <Item glyph="bug" label="Report a Bug" onClick={pick(() => window.open(bugReportUrl(), '_blank', 'noopener'))} />
          </div>
          {(onSignOut || onExit) && (
            <div className={styles.menuGroup}>
              {onSignOut && <Item glyph="signOut" label="Sign Out" onClick={pick(askSignOut(onSignOut))} />}
              {onExit && <Item glyph="exit" label="Exit" onClick={pick(onExit)} />}
            </div>
          )}
          <div className={styles.menuFoot}>
            <a href={siteUrl('/terms/')} target="_blank" rel="noreferrer">
              Terms of Service
            </a>
            <a href={siteUrl('/privacy/')} target="_blank" rel="noreferrer">
              Privacy Policy
            </a>
          </div>
        </div>
      )}
    </div>
  )
}
