// MePanel — the panel the Me button opens: who the player is, the wallet address and the invite link to
// copy, the player's own pages, and the ways out. The bar places it and closes it; every action comes in
// by props, and a row whose action the bar does not pass is not drawn.

import { useEffect, useRef, useState } from 'react'
import { Avatar, MenuList, MenuPanel, MenuRow, showConfirm, showToast } from '../../../design'
import { isAddress, splitName, userColor } from '@explorer-core/lib/identity'
import type { Profile } from '@explorer-core/engine/protocol'
import backpackIcon from '../../../assets/sidebar-rail/me-backpack.svg?no-inline'
import profileIcon from '../../../assets/sidebar-rail/me-profile.svg?no-inline'
import badgesIcon from '../../../assets/sidebar-rail/me-badges.svg?no-inline'
import signOutIcon from '../../../assets/sidebar-rail/me-sign-out.svg?no-inline'
import exitIcon from '../../../assets/sidebar-rail/me-exit.svg?no-inline'
import copyIcon from '../../../assets/sidebar-rail/me-copy.svg?no-inline'
import styles from './MePanel.module.css'

export interface MePanelProps {
  /** The local player; the head is left out until the profile has loaded. */
  profile: Pick<Profile, 'address' | 'name' | 'picture' | 'hasClaimedName' | 'nameColor'> | null
  /** The key bound to the Backpack, drawn as "[I]" after the label. */
  backpackKey?: string
  /** The player's invite link, copied by INVITE FRIENDS; the button is left out without one. */
  inviteLink?: string
  onBackpack: () => void
  /** Open the player's own passport. */
  onProfile: () => void
  /** Open the player's own passport on its badges. */
  onBadges?: () => void
  /** Leave the world for the start screen, signed out; the row asks first. The bar passes it for a sign-in on the web the player made, a guest's included. */
  onSignOut?: () => void
  /** Leave the application. */
  onExit?: () => void
  /** Called after a row's action, so the bar closes the panel. */
  onClose: () => void
}

const COPIED_MS = 1200

// The art keeps its own colours (the red Exit), so it is drawn as a picture at its own size.
const art = (src: string): React.JSX.Element => <img src={src} alt="" draggable={false} />

/** "0x10e...a7a92": the first five and the last five characters. */
const shortWallet = (address: string): string => `${address.slice(0, 5)}...${address.slice(-5)}`

export function MePanel({ profile, backpackKey, inviteLink, onBackpack, onProfile, onBadges, onSignOut, onExit, onClose }: MePanelProps): React.JSX.Element {
  const [copied, setCopied] = useState(false)
  const copiedTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  useEffect(() => () => clearTimeout(copiedTimer.current), [])

  const pick = (act: () => void) => (): void => {
    act()
    onClose()
  }
  // Sign Out asks first. The panel stays open under the question, so Cancel and Escape hand the
  // focus back to the row; only the confirmed sign-out closes the panel.
  const askSignOut = (signOut: () => void) => (e: React.MouseEvent<HTMLButtonElement>): void => {
    e.currentTarget.focus()
    void showConfirm({ title: 'Sign out?', body: 'You leave the world and return to the start screen.', cancelLabel: 'Cancel', confirmLabel: 'Sign Out' }).then((confirmed) => {
      if (!confirmed) return
      signOut()
      onClose()
    })
  }
  const copy = (address: string): void => {
    navigator.clipboard?.writeText(address).then(
      () => {
        setCopied(true)
        clearTimeout(copiedTimer.current)
        copiedTimer.current = setTimeout(() => setCopied(false), COPIED_MS)
      },
      () => {}
    )
  }

  const copyInvite = (link: string): void => {
    navigator.clipboard?.writeText(link).then(
      () => showToast('Link copied!', { key: 'invite-link' }),
      () => {}
    )
  }

  const color = profile != null ? userColor(profile.address, profile.name, profile.hasClaimedName, profile.nameColor) : undefined
  const wallet = profile != null && isAddress(profile.address) ? profile.address : null
  const { base, tag } = splitName(profile?.name ?? '')
  // an unclaimed name is told apart by the end of its wallet, as in every other list of players
  const shownTag = tag !== '' ? tag : profile != null && !profile.hasClaimedName && wallet != null ? `#${wallet.slice(-4)}` : ''
  const footer = onSignOut != null || onExit != null

  return (
    <MenuPanel label="Me">
      {profile != null && (
        <>
          <div className={styles.head}>
            {/* signed in and connected: the picture carries the connection dot */}
            <Avatar src={profile.picture} name={profile.name} color={color} size={53} status="online" dotPosition="top" className={styles.pic} />
            <div className={styles.info}>
              <p className={styles.name} style={{ '--me-name': color } as React.CSSProperties}>
                <span className={styles.base}>{base}</span>
                {shownTag !== '' && <span className={styles.tag}>{shownTag}</span>}
              </p>
              {wallet != null && (
                <button type="button" className={styles.wallet} title="Copy address" onClick={() => copy(wallet)}>
                  <span aria-live="polite">{copied ? 'Copied' : shortWallet(wallet)}</span>
                  <span className={styles.copy} aria-hidden="true">
                    {art(copyIcon)}
                  </span>
                </button>
              )}
              {inviteLink != null && (
                <button type="button" className={`${styles.wallet} ${styles.invite}`} onClick={() => copyInvite(inviteLink)}>
                  INVITE FRIENDS
                </button>
              )}
            </div>
          </div>
          <div className={styles.rule} role="separator" />
        </>
      )}
      <MenuList>
        <MenuRow icon={art(backpackIcon)} label="Backpack" hint={backpackKey} onClick={pick(onBackpack)} />
        <MenuRow icon={art(profileIcon)} label="Profile" onClick={pick(onProfile)} />
        {onBadges != null && <MenuRow icon={art(badgesIcon)} label="Badges" onClick={pick(onBadges)} />}
      </MenuList>
      {footer && (
        <>
          <div className={styles.rule} role="separator" />
          <MenuList>
            {onSignOut != null && <MenuRow icon={art(signOutIcon)} label="Sign Out" aria-haspopup="dialog" onClick={askSignOut(onSignOut)} />}
            {onExit != null && <MenuRow icon={art(exitIcon)} label="Exit" onClick={pick(onExit)} />}
          </MenuList>
        </>
      )}
    </MenuPanel>
  )
}
