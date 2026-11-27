// MainMenuShell — the shared full-screen menu chrome (dark top bar + accent line +
// body). Every full-screen menu page (Settings, Backpack, …) renders inside this so
// the top bar is identical and consistent. Pages pass their content as children.

import { useContext, useEffect, useState } from 'react'
import { Icon, type IconName } from '../../design'
import { keyHintFor, useBindingsSnapshot } from '../../lib/bindingLabels'
import { EDITOR_BUILD } from '@explorer-core/features/editorHost/config'
import { EditorOffered } from '../editorHost/EditorOffered'
import { ProfileChip } from './ProfileChip'
import { MenuBackdrop } from './MenuBackdrop'
import { MenuPresentationContext, type MenuPresentation } from './MenuPresentation'
import styles from './MainMenuShell.module.css'
import onBackpack from '../../assets/menu-on/backpack.webp'
import onEvents from '../../assets/menu-on/events.webp'
import onPlaces from '../../assets/menu-on/places.webp'
import onCommunities from '../../assets/menu-on/communities.webp'
import onShop from '../../assets/menu-on/shop.webp'
import onMap from '../../assets/menu-on/map.webp'
import onSettings from '../../assets/menu-on/settings.webp'
import onGallery from '../../assets/menu-on/gallery.webp'
import { Button, DclLogo } from '@dcl/one-ui'

// The selected tab's full-colour artwork, and its display size (the map and settings art sit in a
// larger canvas).
const SELECTED_ART: Record<string, [string, number]> = {
  backpack: [onBackpack, 36], events: [onEvents, 36], places: [onPlaces, 36], communities: [onCommunities, 36],
  shop: [onShop, 36], map: [onMap, 46], settings: [onSettings, 46], gallery: [onGallery, 36]
}

// Menu shells currently mounted. A shell that renders while another is mounted is a page switch and
// plays no entrance; one that renders with none mounted is a fresh open.
let openShells = 0
// A shell that mounts within this many ms of the last one closing is a page switch, not a fresh open.
const SWITCH_GAP_MS = 200
let lastShellClosedAt = Number.NEGATIVE_INFINITY

export interface MenuItem {
  label: string
  icon: IconName
  /** Engine SystemAction whose live binding renders as the [K] hint. */
  hotkey?: string
  /** React page id this item opens. */
  page: string
}

// The menu pages we support (others hidden). Matches the Figma nav bar
// (icon + LABEL [shortcut]). Every item is now a React page.
export const MENU_ITEMS: MenuItem[] = [
  { label: 'Events', icon: 'events', hotkey: 'Events', page: 'events' },
  { label: 'Places', icon: 'places', hotkey: 'Places', page: 'places' },
  { label: 'Shop', icon: 'marketplace', page: 'shop' },
  { label: 'Communities', icon: 'communities', hotkey: 'Communities', page: 'communities' },
  { label: 'Map', icon: 'map', hotkey: 'Map', page: 'map' },
  { label: 'Backpack', icon: 'backpack', hotkey: 'Backpack', page: 'backpack' },
  { label: 'Gallery', icon: 'gallery', hotkey: 'Gallery', page: 'gallery' },
  { label: 'Settings', icon: 'settings', hotkey: 'Settings', page: 'settings' }
]

// The scene editor's home (editorHost/CreatePage).
const CREATE_ITEM: MenuItem = { label: 'Create', icon: 'create', page: 'create' }

export function MainMenuShell({
  active,
  profileName,
  profilePicture,
  profileAddress,
  profileClaimed,
  onNavigate,
  onClose,
  transparentBody = false,
  presentation,
  children
}: {
  /** page id of the active React page, e.g. 'settings'. */
  active: string
  profileName?: string
  profilePicture?: string
  profileAddress?: string
  profileClaimed?: boolean
  onNavigate: (page: string) => void
  onClose: () => void
  /** Body becomes a pass-through hole for an engine-rendered view (map/avatar). */
  transparentBody?: boolean
  /** `lobbyCard`: a card over the dimmed lobby with only a close button. Defaults to what the presenter set in MenuPresentationContext. */
  presentation?: MenuPresentation
  children: React.ReactNode
}): React.JSX.Element {
  // Animate the entrance only on a fresh open (no other shell mounted), not on page switches.
  const [animate] = useState(() => openShells === 0 && Date.now() - lastShellClosedAt > SWITCH_GAP_MS)
  const bindingsSnap = useBindingsSnapshot()
  const offered = useContext(EditorOffered)
  const presented = useContext(MenuPresentationContext)
  const card = (presentation ?? presented) === 'lobbyCard'
  const items = EDITOR_BUILD && offered ? [...MENU_ITEMS, CREATE_ITEM] : MENU_ITEMS
  useEffect(() => {
    openShells++
    return () => {
      openShells--
      if (openShells === 0) lastShellClosedAt = Date.now()
    }
  }, [])

  const close = (
    <button type="button" className={`${styles.close} ${card ? styles.cardClose : ''}`.trim()} aria-label="Close" onClick={onClose}>
      <span className={styles.closeIcon} aria-hidden="true" />
    </button>
  )
  if (card) {
    return (
      <div
        className={`${styles.overlay} ${styles.card} ${animate ? styles.animateIn : ''} ${transparentBody ? styles.overlayPass : ''}`.trim()}
        data-presentation="lobbyCard"
      >
        <header className={styles.cardBar}>{close}</header>
        {!transparentBody && <MenuBackdrop className={styles.backdropSlot} watermark={active === 'settings' ? 'settings' : undefined} />}
        <div className={`${styles.body} ${transparentBody ? styles.bodyTransparent : ''}`.trim()}>{children}</div>
      </div>
    )
  }
  return (
    <div className={`${styles.overlay} ${animate ? styles.animateThroughDark : ''} ${transparentBody ? styles.overlayPass : ''}`.trim()}>
      <header className={styles.topbar}>
        <div className={styles.brand}>
          <DclLogo size={25} />
          <span className={styles.brandName}>Decentraland</span>
        </div>
        <Button appearance="hud" variant="accent" size="header" className={styles.lobby} onClick={() => onNavigate('lobby')}>
          <Icon name="lobby" size={20} />
          Lobby
        </Button>
        <nav className={styles.menu}>
          {items.map((m) => {
            const shortcut = m.hotkey != null ? keyHintFor(bindingsSnap, m.hotkey) : undefined
            return (
              <button
                key={m.label}
                type="button"
                className={`${styles.menuItem} ${m.page === active ? styles.menuActive : ''}`.trim()}
                data-page={m.page}
                aria-current={m.page === active ? 'page' : undefined}
                onClick={() => m.page !== active && onNavigate(m.page)}
              >
                <span className={styles.menuIcon}>
                  {m.page === active && SELECTED_ART[m.page] != null ? (
                    <img src={SELECTED_ART[m.page][0]} alt="" width={SELECTED_ART[m.page][1]} height={SELECTED_ART[m.page][1]} />
                  ) : (
                    <Icon name={m.icon} size={28} />
                  )}
                </span>
                <span className={styles.menuLabel}>
                  {m.label}
                  {shortcut && <span className={styles.menuKey}> [{shortcut}]</span>}
                </span>
              </button>
            )
          })}
        </nav>
        <div className={styles.end}>
        {profileName && (
          <ProfileChip
            name={profileName}
            picture={profilePicture}
            address={profileAddress}
            claimed={profileClaimed}
            onViewProfile={() => onNavigate('profile')}
            onExit={onClose}
          />
        )}
        {close}
        </div>
      </header>
      <div className={styles.accent} />
      {!transparentBody && <MenuBackdrop className={styles.backdropSlot} watermark={active === 'settings' ? 'settings' : undefined} />}
      <div className={`${styles.body} ${transparentBody ? styles.bodyTransparent : ''}`.trim()}>{children}</div>
      {animate && <div className={styles.darkFrame} aria-hidden="true" />}
    </div>
  )
}
