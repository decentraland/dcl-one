import { useState, useSyncExternalStore, type ComponentType, type ReactNode } from 'react'
import { NavBar } from '@dcl/one-ui'
import { Button } from '../shared/Button/Button'
import { Modal } from '../shared/Modal/Modal'
import {
  GeneralCopy,
  GeneralLogout,
  GeneralMana,
  GeneralPolygonMana,
  type IconProps
} from '../../../foundations/icons'
import { followHandlers, type ChromeLinkClick } from './follow'
import styles from './SitesNavbar.module.css'

/** `id` names the link in analytics: the last segment of its label's i18n key. */
type NavLink = { id?: string; label: ReactNode; href: string; external?: boolean }
type NavMenu = { id: string; label: ReactNode; items: readonly NavLink[]; active?: boolean }

/** Data the navbar shows while it is read: unknown until it loads, and on failure. */
type Loadable<T> = { status: 'loading' } | { status: 'error' } | { status: 'ready'; value: T }

type NavNotification = {
  id: string
  title: ReactNode
  body?: ReactNode
  time: ReactNode
  href?: string
  unread: boolean
}

type SitesNavbarFullProps = {
  /** The default variant: menus, then the account or Sign In. */
  variant?: 'full'
  /** The home page: transparent over the hero and scrolling away from desktop up, as source. */
  overlay?: boolean
  home: { href: string; label: string }
  menus: readonly NavMenu[]
  learn: NavLink
  labels: {
    openMenu: string
    closeMenu: string
    notifications: string
    userMenu: string
    copyAddress: string
    addressCopied: ReactNode
    walletAddress: ReactNode
    logout: ReactNode
    switchAccount: ReactNode
    loading: ReactNode
    notificationsTitle: ReactNode
    notificationsEmpty: ReactNode
    notificationsError: ReactNode
    manaError: ReactNode
    /** Read out before an unread notification; not shown, the row's style marks it. */
    unread: ReactNode
  }
  /** A link was followed, e.g. for analytics. Links without an `id` are not reported. */
  onLinkClick?: (link: ChromeLinkClick) => void
} & (SitesNavbarAccount | SitesNavbarSignedOut)

/** Signed in: credits, notifications and the account card. */
type SitesNavbarAccount = {
  /** Shown only once known: a 0 while loading or on a failed read would claim the wallet has none. */
  credits: { href: string; state: Loadable<string>; label: (count: string) => string }
  notifications: {
    unread: number
    state: Loadable<readonly NavNotification[]>
    /** The panel opened: load the list; what it shows while open is marked read. */
    onOpen: () => void
    /** The panel closed. */
    onClose?: () => void
  }
  user: {
    name: ReactNode
    address: string
    avatarUrl?: string
    links: readonly (NavLink & { icon: ComponentType<IconProps> })[]
    /** MANA on Ethereum and Polygon, formatted. */
    mana: Loadable<{ ethereum: string; polygon: string }>
    /** The card opened: read the balances. */
    onOpen: () => void
    onSignOut?: () => void
    onSwitchAccount?: () => void
  }
  signIn?: never
}

/** Signed out: the same menus, then Sign In in place of the account. */
type SitesNavbarSignedOut = {
  signIn: { label: ReactNode; onClick: () => void }
  credits?: never
  notifications?: never
  user?: never
}

/** The signed-out home page, as source LandingNavbar's minimal variant: logo, wordmark and Sign In
    over the hero; once the visitor scrolls, Sign In gives way to Jump In. */
type SitesNavbarLandingProps = {
  variant: 'landing'
  home: { href: string; label: string }
  signIn: { label: ReactNode; onClick: () => void; busy?: boolean }
  /** Once the visitor scrolls, Sign In gives way to this action. */
  jumpIn?: { label: ReactNode; onClick: () => void }
  /** The logo link was followed, reported as the full navbar reports it. */
  onLinkClick?: (link: ChromeLinkClick) => void
}

/** The signed-out download page, as source's own download header: the logo and Sign In over the
    page, where the page leaves room for them (DownloadPage `reserveHeaderRow`). */
type SitesNavbarCompactProps = {
  variant: 'compact'
  home: { href: string; label: string }
  signIn: { label: ReactNode; onClick: () => void }
}

type SitesNavbarProps = SitesNavbarFullProps | SitesNavbarLandingProps | SitesNavbarCompactProps

const shortAddress = (address: string) => `${address.slice(0, 6)}...${address.slice(-4)}`

function externalProps(link: NavLink) {
  return link.external ? { target: '_blank', rel: 'noopener noreferrer' } : {}
}

function NotificationsList({
  state,
  labels
}: {
  state: SitesNavbarAccount['notifications']['state']
  labels: SitesNavbarFullProps['labels']
}) {
  if (state.status === 'loading') return <p className={styles.panelNote}>{labels.loading}</p>
  if (state.status === 'error')
    return <p className={styles.panelNote}>{labels.notificationsError}</p>
  if (!state.value.length) return <p className={styles.panelNote}>{labels.notificationsEmpty}</p>
  return (
    <ul className={styles.notificationList}>
      {state.value.map((item) => {
        const content = (
          <>
            {item.unread ? <span className={styles.visuallyHidden}>{labels.unread}</span> : null}
            <span className={styles.notificationTitle}>{item.title}</span>
            {item.body ? <span className={styles.notificationBody}>{item.body}</span> : null}
            <span className={styles.notificationTime}>{item.time}</span>
          </>
        )
        return (
          <li
            key={item.id}
            className={
              item.unread ? `${styles.notification} ${styles.unread}` : styles.notification
            }
          >
            {item.href ? (
              <a className={styles.notificationLink} href={item.href}>
                {content}
              </a>
            ) : (
              <div className={styles.notificationLink}>{content}</div>
            )}
          </li>
        )
      })}
    </ul>
  )
}

/** The handlers reporting a followed navbar link, when it has an id to report. */
function linkTracking(
  onLinkClick: SitesNavbarFullProps['onLinkClick'],
  section: string,
  link: { id?: string; href: string }
) {
  const action = link.id
  return followHandlers(
    onLinkClick && action ? () => onLinkClick({ section, action, href: link.href }) : undefined
  )
}

function UserCardBody({
  user,
  labels,
  onLinkClick
}: {
  user: SitesNavbarAccount['user']
  labels: SitesNavbarFullProps['labels']
  onLinkClick: SitesNavbarFullProps['onLinkClick']
}) {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    navigator.clipboard
      ?.writeText(user.address)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
      // Clipboard refused (permissions, insecure context): the address stays readable to copy.
      .catch(() => undefined)
  }
  return (
    <>
      <div className={styles.userHead}>
        <span className={styles.userName}>{user.name}</span>
        <span className={styles.addressLabel}>{labels.walletAddress}</span>
        <span className={styles.address}>
          <span aria-live="polite">
            {copied ? labels.addressCopied : shortAddress(user.address)}
          </span>
          <Button variant="iconOnly" size="sm" aria-label={labels.copyAddress} onClick={copy}>
            <GeneralCopy size={16} aria-hidden="true" />
          </Button>
        </span>
        <div className={styles.mana} aria-busy={user.mana.status === 'loading'}>
          {user.mana.status === 'ready' ? (
            <>
              <span className={styles.manaRow}>
                <GeneralMana size={20} aria-hidden="true" />
                {user.mana.value.ethereum}
              </span>
              <span className={styles.manaRow}>
                <GeneralPolygonMana size={20} aria-hidden="true" />
                {user.mana.value.polygon}
              </span>
            </>
          ) : user.mana.status === 'error' ? (
            <span className={styles.panelNote}>{labels.manaError}</span>
          ) : (
            <span className={styles.panelNote}>{labels.loading}</span>
          )}
        </div>
      </div>
      <hr className={styles.divider} />
      <ul className={styles.userLinks}>
        {user.links.map(({ icon: Icon, ...link }) => (
          <li key={link.href}>
            <a
              className={styles.userLink}
              href={link.href}
              {...externalProps(link)}
              {...linkTracking(onLinkClick, 'user_menu', link)}
            >
              <Icon size={20} aria-hidden="true" />
              {link.label}
            </a>
          </li>
        ))}
      </ul>
      <hr className={styles.divider} />
      {user.onSwitchAccount ? (
        <button type="button" className={styles.logout} onClick={user.onSwitchAccount}>
          <GeneralLogout size={18} aria-hidden="true" />
          {labels.switchAccount}
        </button>
      ) : user.onSignOut ? (
        <button type="button" className={styles.logout} onClick={user.onSignOut}>
          <GeneralLogout size={18} aria-hidden="true" />
          {labels.logout}
        </button>
      ) : null}
    </>
  )
}

type Viewport = 'mobile' | 'desktop' | 'largeDesktop'
const viewport = (): Viewport =>
  window.innerWidth >= 1536 ? 'largeDesktop' : window.innerWidth >= 1024 ? 'desktop' : 'mobile'
const subscribe = (notify: () => void) => {
  window.addEventListener('resize', notify)
  return () => window.removeEventListener('resize', notify)
}
const serverViewport = (): Viewport => 'desktop'

/** Source-owned header geometry and captured content; application callbacks own the account panels. */
function SitesNavbar(props: SitesNavbarProps) {
  const size = useSyncExternalStore(subscribe, viewport, serverViewport)
  const [panel, setPanel] = useState<'notifications' | 'user' | 'menu' | null>(null)
  const full =
    props.variant === undefined || props.variant === 'full'
      ? (props as SitesNavbarFullProps)
      : undefined
  const landing = props.variant === 'landing'
  const overlay = landing || !!full?.overlay
  const user = full?.user
  const close = () => {
    if (panel === 'notifications') full?.notifications?.onClose?.()
    setPanel(null)
  }
  const openNotifications = () => {
    full?.notifications?.onOpen()
    setPanel('notifications')
  }
  const openUser = () => {
    user?.onOpen()
    setPanel('user')
  }
  const discover = full?.menus.find((menu) => menu.id === 'discover')?.items[0]?.href ?? '/events'
  const shop = full?.menus.find((menu) => menu.id === 'shop')?.items[0]?.href ?? '/shop'
  const create = full?.menus.find((menu) => menu.id === 'create')?.items[0]?.href ?? '/create'
  return (
    <>
      <header
        className={[
          styles.generatedHost,
          landing ? styles.generatedLanding : full?.overlay ? styles.generatedOverlay : undefined
        ]
          .filter(Boolean)
          .join(' ')}
      >
        <NavBar
          className={[styles.generatedRoot, overlay ? styles.generatedTransparent : undefined]
            .filter(Boolean)
            .join(' ')}
          viewport={size}
          signedIn={!!user}
          balances="none"
          theme="dark"
          onClick={(event) => {
            const target = event.target as HTMLElement
            const link = target.closest('a')
            if (!link) return
            const href = link.getAttribute('href')
            if (href === props.home.href)
              ('onLinkClick' in props ? props.onLinkClick : undefined)?.({
                section: 'home',
                action: 'logo',
                href
              })
            else if (full) {
              const menu = full.menus.find((item) => item.items[0]?.href === href)
              if (menu?.items[0]?.id)
                full.onLinkClick?.({ section: menu.id, action: menu.items[0].id, href: href! })
              else if (href === full.learn.href)
                full.onLinkClick?.({ section: 'learn', action: 'learn', href })
            }
          }}
          logoHref={props.home.href}
          labelHref={discover}
          label2Href={shop}
          label3Href={create}
          label4Href={full?.learn.href ?? '/blog'}
          onSignIn={props.signIn?.onClick}
          onNotifications={openNotifications}
          onProfile={openUser}
          onMenu={() => setPanel('menu')}
        />
      </header>
      {full && panel === 'notifications' && full.notifications ? (
        <Modal
          title={full.labels.notificationsTitle}
          closeLabel={full.labels.closeMenu}
          onClose={close}
        >
          <NotificationsList state={full.notifications.state} labels={full.labels} />
        </Modal>
      ) : null}
      {full && panel === 'user' && user ? (
        <Modal title={user.name} closeLabel={full.labels.closeMenu} onClose={close}>
          <UserCardBody user={user} labels={full.labels} onLinkClick={full.onLinkClick} />
        </Modal>
      ) : null}
      {full && panel === 'menu' ? (
        <Modal title={full.labels.openMenu} closeLabel={full.labels.closeMenu} onClose={close}>
          <nav>
            {full.menus.map((menu) => (
              <section key={menu.id}>
                <h2>{menu.label}</h2>
                <ul>
                  {menu.items.map((link) => (
                    <li key={link.href}>
                      <a href={link.href} {...externalProps(link)}>
                        {link.label}
                      </a>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </nav>
        </Modal>
      ) : null}
    </>
  )
}
export { SitesNavbar }
export type { Loadable, NavLink, NavMenu, NavNotification, SitesNavbarProps }
