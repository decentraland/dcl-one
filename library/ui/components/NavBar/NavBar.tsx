import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type AnchorHTMLAttributes,
  type ComponentType,
  type FocusEvent as ReactFocusEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject
} from 'react'
import {
  GeneralBell,
  GeneralClose,
  GeneralDecentralandColor,
  GeneralDownArrow,
  GeneralManaColour,
  GeneralPolygonMana
} from '../../foundations/icons/general'
import { ShopCredits } from '../../foundations/icons/shop'
import { breakpoints } from '../../foundations/layout/layout'
import styles from './NavBar.module.css'

export type NavBarLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }

/** Renders internal links (paths starting with a single `/`) with the app's router. */
export type NavBarLinkComponent = ComponentType<NavBarLinkProps>

export type NavBarLink = {
  label: string
  href: string
  /** Marks the link as the current page. */
  active?: boolean
}

/** A top-level section: a link, or a dropdown of links. */
export type NavBarSection =
  | (NavBarLink & { items?: undefined })
  | { label: string; items: readonly NavBarLink[]; active?: boolean; href?: undefined }

export type NavBarAccount = {
  address: string
  name?: string
  /** The account's face image; the Decentraland mark is shown without one. */
  avatarUrl?: string
}

/** Balances are shown only when defined; nothing is shown for a missing value. */
export type NavBarBalances = {
  credits?: number
  mana?: number
  polygonMana?: number
}

export type NavBarAccountAction =
  | { label: string; href: string; onSelect?: undefined }
  | { label: string; onSelect: () => void; href?: undefined }

export type NavBarLabels = {
  navigation: string
  home: string
  signIn: string
  notifications: string
  unreadNotifications: (count: number) => string
  account: (name: string) => string
  openMenu: string
  closeMenu: string
  menu: string
  credits: string
  mana: string
  polygonMana: string
}

export type NavBarProps = {
  sections: readonly NavBarSection[]
  /** Defaults to `/`. */
  logoHref?: string
  /** The signed-in account; signed out when undefined. */
  account?: NavBarAccount
  /** Signed out: the Sign in action. Takes precedence over `signInHref`. */
  onSignIn?: () => void
  signInHref?: string
  balances?: NavBarBalances
  /** Unread notifications; the badge is hidden when 0 or undefined. */
  notificationCount?: number
  /** Signed in: shows the notifications button. */
  onNotificationsClick?: () => void
  /** Signed in: the account menu's entries, for example profile links and sign out. */
  accountActions?: readonly NavBarAccountAction[]
  linkComponent?: NavBarLinkComponent
  labels?: Partial<NavBarLabels>
  /** BCP 47 locale for balance numbers; defaults to `en-US`. Pass the page's locale so server and client agree. */
  locale?: string
  className?: string
}

const defaultLabels: NavBarLabels = {
  navigation: 'Main',
  home: 'Decentraland home',
  signIn: 'Sign in',
  notifications: 'Notifications',
  unreadNotifications: (count) => `Notifications, ${count} unread`,
  account: (name) => `Account: ${name}`,
  openMenu: 'Open menu',
  closeMenu: 'Close menu',
  menu: 'Menu',
  credits: 'Credits',
  mana: 'MANA',
  polygonMana: 'Polygon MANA'
}

const BADGE_MAX = 99
// Where the section links replace the menu button; the module's media queries use the same value.
const DESKTOP_QUERY = `(min-width: ${breakpoints.desktop}px)`
const FOCUSABLE = 'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])'

const isInternalHref = (href: string) => href.startsWith('/') && !href.startsWith('//')

const shortAddress = (address: string) =>
  address.length > 10 ? `${address.slice(0, 6)}...${address.slice(-4)}` : address

const unreadCount = (count: number | undefined) =>
  count !== undefined && Number.isFinite(count) && count >= 1 ? Math.floor(count) : 0

const hasValue = (value: number | undefined): value is number =>
  value !== undefined && Number.isFinite(value)

type LinkRenderer = (props: NavBarLinkProps) => ReactNode

const linkRenderer =
  (LinkComponent: NavBarLinkComponent | undefined): LinkRenderer =>
  (props) =>
    LinkComponent && isInternalHref(props.href) ? <LinkComponent {...props} /> : <a {...props} />

/** Closes an open popup on Escape (returning focus to its trigger), on an outside press and when focus leaves it. */
function usePopup(rootRef: RefObject<HTMLElement | null>, triggerRef: RefObject<HTMLButtonElement | null>) {
  const [open, setOpen] = useState(false)
  useEffect(() => {
    if (!open) return
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false)
    }
    document.addEventListener('pointerdown', onPointerDown)
    return () => document.removeEventListener('pointerdown', onPointerDown)
  }, [open, rootRef])
  const onKeyDown = (event: ReactKeyboardEvent) => {
    if (event.key !== 'Escape' || !open) return
    event.stopPropagation()
    setOpen(false)
    triggerRef.current?.focus()
  }
  const onBlur = (event: ReactFocusEvent) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false)
  }
  return { open, setOpen, onKeyDown, onBlur }
}

function SectionDropdown({
  section,
  renderLink
}: {
  section: Extract<NavBarSection, { items: readonly NavBarLink[] }>
  renderLink: LinkRenderer
}) {
  const rootRef = useRef<HTMLLIElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const listId = useId()
  const { open, setOpen, onKeyDown, onBlur } = usePopup(rootRef, triggerRef)
  const active = section.active || section.items.some((item) => item.active)
  return (
    <li
      ref={rootRef}
      className={styles.section}
      onKeyDown={onKeyDown}
      onBlur={onBlur}
    >
      <button
        ref={triggerRef}
        type="button"
        className={styles.tab}
        data-active={active || undefined}
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen((value) => !value)}
      >
        {section.label}
        <GeneralDownArrow className={styles.chevron} size={20} aria-hidden="true" />
      </button>
      <ul id={listId} className={styles.dropdown} hidden={!open}>
        {section.items.map((item) => (
          <li key={item.href + item.label}>
            {renderLink({
              href: item.href,
              className: styles.dropdownLink,
              'aria-current': item.active ? 'page' : undefined,
              onClick: () => setOpen(false),
              children: item.label
            })}
          </li>
        ))}
      </ul>
    </li>
  )
}

function Avatar({ account }: { account: NavBarAccount }) {
  const [failed, setFailed] = useState<string | undefined>(undefined)
  const url = account.avatarUrl && failed !== account.avatarUrl ? account.avatarUrl : undefined
  return (
    <span className={styles.avatar}>
      {url ? (
        <img className={styles.avatarImage} src={url} alt="" onError={() => setFailed(url)} />
      ) : (
        <GeneralDecentralandColor className={styles.avatarImage} aria-hidden="true" />
      )}
    </span>
  )
}

function Balances({
  balances,
  labels,
  format
}: {
  balances: NavBarBalances
  labels: NavBarLabels
  format: Intl.NumberFormat
}) {
  const entries = [
    { key: 'credits', value: balances.credits, label: labels.credits, Icon: ShopCredits },
    { key: 'mana', value: balances.mana, label: labels.mana, Icon: GeneralManaColour },
    { key: 'polygonMana', value: balances.polygonMana, label: labels.polygonMana, Icon: GeneralPolygonMana }
  ].filter((entry): entry is typeof entry & { value: number } => hasValue(entry.value))
  if (entries.length === 0) return null
  return (
    <ul className={styles.balances}>
      {entries.map(({ key, value, label, Icon }) => (
        <li key={key} className={styles.balance} data-balance={key}>
          <Icon className={styles.balanceIcon} size={20} aria-hidden="true" />
          <span className={styles.visuallyHidden}>{label}: </span>
          {format.format(value)}
        </li>
      ))}
    </ul>
  )
}

function NotificationsButton({
  count,
  labels,
  onClick
}: {
  count: number
  labels: NavBarLabels
  onClick: () => void
}) {
  return (
    <button
      type="button"
      className={styles.iconButton}
      aria-label={count > 0 ? labels.unreadNotifications(count) : labels.notifications}
      onClick={onClick}
    >
      <GeneralBell size={24} aria-hidden="true" />
      {count > 0 ? (
        <span className={styles.badge} aria-hidden="true">
          {count > BADGE_MAX ? `${BADGE_MAX}+` : count}
        </span>
      ) : null}
    </button>
  )
}

function AccountActionList({
  actions,
  renderLink,
  onDone,
  onActionDone = onDone,
  className
}: {
  actions: readonly NavBarAccountAction[]
  renderLink: LinkRenderer
  /** After a link is followed. */
  onDone: () => void
  /** After a button action; defaults to onDone. */
  onActionDone?: () => void
  className: string | undefined
}) {
  return (
    <ul className={className}>
      {actions.map((action) => (
        <li key={`${action.href ?? ''}${action.label}`}>
          {action.href !== undefined ? (
            renderLink({ href: action.href, className: styles.menuItem, onClick: onDone, children: action.label })
          ) : (
            <button
              type="button"
              className={styles.menuItem}
              onClick={() => {
                onActionDone()
                action.onSelect()
              }}
            >
              {action.label}
            </button>
          )}
        </li>
      ))}
    </ul>
  )
}

function AccountMenu({
  account,
  actions,
  labels,
  renderLink
}: {
  account: NavBarAccount
  actions: readonly NavBarAccountAction[]
  labels: NavBarLabels
  renderLink: LinkRenderer
}) {
  const rootRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const menuId = useId()
  const { open, setOpen, onKeyDown, onBlur } = usePopup(rootRef, triggerRef)
  const displayName = account.name || shortAddress(account.address)
  return (
    <div ref={rootRef} className={styles.account} onKeyDown={onKeyDown} onBlur={onBlur}>
      <button
        ref={triggerRef}
        type="button"
        className={styles.accountTrigger}
        aria-label={labels.account(displayName)}
        aria-expanded={open}
        aria-controls={menuId}
        onClick={() => setOpen((value) => !value)}
      >
        <Avatar account={account} />
      </button>
      <div id={menuId} className={styles.accountMenu} hidden={!open}>
        <p className={styles.accountName}>{displayName}</p>
        {account.name ? (
          <p className={styles.accountAddress} title={account.address}>
            {shortAddress(account.address)}
          </p>
        ) : null}
        {actions.length > 0 ? (
          <AccountActionList
            actions={actions}
            renderLink={renderLink}
            onDone={() => setOpen(false)}
            className={styles.menuList}
          />
        ) : null}
      </div>
    </div>
  )
}

function Drawer({
  id,
  titleId,
  onClose,
  labels,
  children
}: {
  id: string
  titleId: string
  onClose: () => void
  labels: NavBarLabels
  children: ReactNode
}) {
  const panelRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const panel = panelRef.current
    panel?.querySelector<HTMLElement>(FOCUSABLE)?.focus()
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'
    const desktop = typeof window.matchMedia === 'function' ? window.matchMedia(DESKTOP_QUERY) : undefined
    const onDesktop = (event: MediaQueryListEvent) => event.matches && onClose()
    desktop?.addEventListener('change', onDesktop)
    return () => {
      document.body.style.overflow = overflow
      desktop?.removeEventListener('change', onDesktop)
    }
  }, [onClose])
  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation()
      onClose()
      return
    }
    if (event.key !== 'Tab') return
    const focusable = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE))
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (!first || !last) return
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }
  return (
    <div className={styles.drawerLayer}>
      <div className={styles.backdrop} aria-hidden="true" onClick={onClose} />
      <div
        ref={panelRef}
        id={id}
        className={styles.drawer}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <div className={styles.drawerHeader}>
          <h2 id={titleId} className={styles.drawerTitle}>
            {labels.menu}
          </h2>
          <button type="button" className={styles.iconButton} aria-label={labels.closeMenu} onClick={onClose}>
            <GeneralClose size={24} aria-hidden="true" />
          </button>
        </div>
        {children}
      </div>
    </div>
  )
}

export function NavBar({
  sections,
  logoHref = '/',
  account,
  onSignIn,
  signInHref,
  balances,
  notificationCount,
  onNotificationsClick,
  accountActions = [],
  linkComponent,
  labels: labelOverrides,
  locale = 'en-US',
  className
}: NavBarProps) {
  const labels = { ...defaultLabels, ...labelOverrides }
  const format = useMemo(() => new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }), [locale])
  const renderLink = linkRenderer(linkComponent)
  const [drawerOpen, setDrawerOpen] = useState(false)
  const menuButtonRef = useRef<HTMLButtonElement>(null)
  const drawerId = useId()
  const drawerTitleId = useId()
  const count = unreadCount(notificationCount)
  const closeDrawer = useCallback(() => {
    setDrawerOpen(false)
    menuButtonRef.current?.focus()
  }, [])

  const signIn = onSignIn ? (
    <button type="button" className={styles.signIn} onClick={onSignIn}>
      {labels.signIn}
    </button>
  ) : signInHref !== undefined ? (
    renderLink({ href: signInHref, className: styles.signIn, children: labels.signIn })
  ) : null

  return (
    <nav className={className ? `${styles.root} ${className}` : styles.root} aria-label={labels.navigation}>
      <div className={styles.start}>
        {renderLink({
          href: logoHref,
          className: styles.logo,
          'aria-label': labels.home,
          children: <GeneralDecentralandColor className={styles.logoMark} aria-hidden="true" />
        })}
        <ul className={styles.sections}>
          {sections.map((section) =>
            section.items ? (
              <SectionDropdown key={`d${section.label}`} section={section} renderLink={renderLink} />
            ) : (
              <li key={`${section.href}${section.label}`} className={styles.section}>
                {renderLink({
                  href: section.href,
                  className: styles.tab,
                  'aria-current': section.active ? 'page' : undefined,
                  children: section.label
                })}
              </li>
            )
          )}
        </ul>
      </div>
      <div className={styles.end}>
        {account ? (
          <>
            {balances ? (
              <div className={styles.barBalances}>
                <Balances balances={balances} labels={labels} format={format} />
              </div>
            ) : null}
            {onNotificationsClick ? (
              <NotificationsButton count={count} labels={labels} onClick={onNotificationsClick} />
            ) : null}
            <AccountMenu account={account} actions={accountActions} labels={labels} renderLink={renderLink} />
          </>
        ) : (
          signIn
        )}
        <button
          ref={menuButtonRef}
          type="button"
          className={styles.menuButton}
          aria-label={labels.openMenu}
          aria-expanded={drawerOpen}
          aria-controls={drawerOpen ? drawerId : undefined}
          aria-haspopup="dialog"
          onClick={() => setDrawerOpen(true)}
        >
          <span className={styles.menuBars} aria-hidden="true" />
        </button>
      </div>
      {drawerOpen ? (
        <Drawer id={drawerId} titleId={drawerTitleId} onClose={closeDrawer} labels={labels}>
          <ul className={styles.drawerSections}>
            {sections.map((section) => (
              <li key={`${section.href ?? 'd'}${section.label}`}>
                {section.items ? (
                  <>
                    <p className={styles.drawerGroup}>{section.label}</p>
                    <ul className={styles.drawerItems}>
                      {section.items.map((item) => (
                        <li key={item.href + item.label}>
                          {renderLink({
                            href: item.href,
                            className: styles.drawerLink,
                            'aria-current': item.active ? 'page' : undefined,
                            onClick: () => setDrawerOpen(false),
                            children: item.label
                          })}
                        </li>
                      ))}
                    </ul>
                  </>
                ) : (
                  renderLink({
                    href: section.href,
                    className: styles.drawerLink,
                    'aria-current': section.active ? 'page' : undefined,
                    onClick: () => setDrawerOpen(false),
                    children: section.label
                  })
                )}
              </li>
            ))}
          </ul>
          {account ? (
            <div className={styles.drawerAccount}>
              <div className={styles.drawerIdentity}>
                <Avatar account={account} />
                <span className={styles.accountName}>{account.name || shortAddress(account.address)}</span>
              </div>
              {balances ? <Balances balances={balances} labels={labels} format={format} /> : null}
              {onNotificationsClick ? (
                <button
                  type="button"
                  className={styles.menuItem}
                  onClick={() => {
                    closeDrawer()
                    onNotificationsClick()
                  }}
                >
                  {count > 0 ? labels.unreadNotifications(count) : labels.notifications}
                </button>
              ) : null}
              {accountActions.length > 0 ? (
                <AccountActionList
                  actions={accountActions}
                  renderLink={renderLink}
                  onDone={() => setDrawerOpen(false)}
                  onActionDone={closeDrawer}
                  className={styles.menuList}
                />
              ) : null}
            </div>
          ) : signIn ? (
            <div className={styles.drawerAccount}>{signIn}</div>
          ) : null}
        </Drawer>
      ) : null}
    </nav>
  )
}
