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
  GeneralMenu,
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

/** The Sign in action: a link or a callback. */
export type NavBarSignIn =
  { href: string; onSelect?: undefined } | { onSelect: () => void; href?: undefined }

export type NavBarNotifications = {
  /** Unread notifications; the badge is hidden when 0, negative or not finite. */
  count?: number
  onSelect: () => void
}

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

/** Slot content owns its labels, loading state and events. Controls remount when moved between header and drawer. */
type NavBarCommonProps = {
  sections: readonly NavBarSection[]
  /** Defaults to `/`. */
  logoHref?: string
  balances?: NavBarBalances
  /** Standard foundation surface or black-to-transparent overlay. The app owns layout positioning. */
  surface?: 'solid' | 'overlay'
  /** App-owned balance action beside numeric balances, rendered in the header or open drawer, never both. */
  balanceAction?: ReactNode
  /** Replaces the standard account/sign-in UI. null/undefined retain defaults; false hides them. Rendered once in the desktop header or open drawer. */
  accountControl?: ReactNode
  /** Secondary navigation in the open drawer. The callback lets app-owned links close it after navigation. */
  mobileSecondaryNavigation?: ReactNode | ((context: { closeDrawer: () => void }) => ReactNode)
  /** Signed in: the notifications button. */
  notifications?: NavBarNotifications
  /** Signed in: the account menu's entries, for example profile links and sign out. */
  accountActions?: readonly NavBarAccountAction[]
  linkComponent?: NavBarLinkComponent
  labels?: Partial<NavBarLabels>
  /**
   * BCP 47 locale for balance numbers; defaults to `en-US`, which is also used for a locale `Intl` rejects. Pass the
   * page's locale so server and client agree.
   */
  locale?: string
  className?: string
}

/** Signed in with an `account`; signed out without one, which then requires a `signIn` action. */
export type NavBarProps = NavBarCommonProps &
  (
    | { account: NavBarAccount; signIn?: NavBarSignIn }
    | { account?: undefined; signIn: NavBarSignIn }
  )

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
const FOCUSABLE = 'a[href], button, input, select, textarea, [tabindex]'
const hasSlot = (node: ReactNode) =>
  node !== null && node !== undefined && typeof node !== 'boolean'

function drawerFocusables(panel: HTMLElement): HTMLElement[] {
  return Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((element) => {
    if (
      element.tabIndex < 0 ||
      element.matches(':disabled, input[type="hidden"]') ||
      element.closest('[hidden], [inert]')
    )
      return false
    for (
      let parent: HTMLElement | null = element;
      parent && parent !== panel;
      parent = parent.parentElement
    ) {
      const style = window.getComputedStyle(parent)
      if (style.display === 'none' || style.visibility === 'hidden') return false
    }
    return true
  })
}

// URL parsing drops tabs and newlines and reads `\\` as `/` in http(s) URLs, so `/\\host` and `/\t/host` are
// protocol-relative too: only a single `/` followed by anything else is a path on this origin.
const isInternalHref = (href: string) => {
  const url = href.replace(/[\t\n\r]/g, '')
  return url.startsWith('/') && url[1] !== '/' && url[1] !== '\\'
}

const numberFormat = (locale: string) => {
  try {
    return new Intl.NumberFormat(locale, { maximumFractionDigits: 2 })
  } catch {
    return new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 })
  }
}

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
function usePopup(
  rootRef: RefObject<HTMLElement | null>,
  triggerRef: RefObject<HTMLButtonElement | null>
) {
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
    // Only focus moving elsewhere closes: a null relatedTarget is a click on non-focusable content (or Safari not
    // focusing a clicked link), and outside presses are the pointerdown handler's.
    const next = event.relatedTarget as Node | null
    if (next && !event.currentTarget.contains(next)) setOpen(false)
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
    <li ref={rootRef} className={styles.section} onKeyDown={onKeyDown} onBlur={onBlur}>
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
        {section.items.map((item, index) => (
          <li key={index}>
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
    {
      key: 'polygonMana',
      value: balances.polygonMana,
      label: labels.polygonMana,
      Icon: GeneralPolygonMana
    }
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
  className
}: {
  actions: readonly NavBarAccountAction[]
  renderLink: LinkRenderer
  onDone: () => void
  className: string | undefined
}) {
  return (
    <ul className={className}>
      {actions.map((action, index) => (
        <li key={index}>
          {action.href !== undefined ? (
            renderLink({
              href: action.href,
              className: styles.menuItem,
              onClick: onDone,
              children: action.label
            })
          ) : (
            <button
              type="button"
              className={styles.menuItem}
              onClick={() => {
                onDone()
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
  onDesktop,
  labels,
  children
}: {
  id: string
  titleId: string
  onClose: () => void
  /** The viewport reached desktop, where the menu button that would take focus back is hidden. */
  onDesktop: () => void
  labels: NavBarLabels
  children: ReactNode
}) {
  const panelRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const panel = panelRef.current
    if (panel) drawerFocusables(panel)[0]?.focus()
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'
    const desktop =
      typeof window.matchMedia === 'function' ? window.matchMedia(DESKTOP_QUERY) : undefined
    const onChange = (event: MediaQueryListEvent) => event.matches && onDesktop()
    desktop?.addEventListener('change', onChange)
    // On the document, so Escape and the Tab wrap still work when focus has fallen out of the panel to <body>.
    const onKeyDown = (event: KeyboardEvent) => {
      // App-owned controls and portaled dialogs may already have handled this key.
      if (event.defaultPrevented) return
      if (event.key === 'Escape') {
        event.preventDefault()
        onClose()
        return
      }
      if (event.key !== 'Tab' || !panel) return
      const focusable = drawerFocusables(panel)
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (!first || !last) return
      const inside = panel.contains(document.activeElement)
      if (event.shiftKey && (!inside || document.activeElement === first)) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (!inside || document.activeElement === last)) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => {
      document.body.style.overflow = overflow
      desktop?.removeEventListener('change', onChange)
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [onClose, onDesktop])
  return (
    <div className={styles.drawerLayer}>
      <div
        className={styles.backdrop}
        aria-hidden="true"
        data-testid="navbar-backdrop"
        onClick={onClose}
      />
      <div
        ref={panelRef}
        id={id}
        className={styles.drawer}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        <div className={styles.drawerHeader}>
          <h2 id={titleId} className={styles.drawerTitle}>
            {labels.menu}
          </h2>
          <button
            type="button"
            className={styles.iconButton}
            aria-label={labels.closeMenu}
            onClick={onClose}
          >
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
  signIn: signInAction,
  balances,
  surface = 'solid',
  balanceAction,
  accountControl,
  mobileSecondaryNavigation,
  notifications,
  accountActions = [],
  linkComponent,
  labels: labelOverrides,
  locale = 'en-US',
  className
}: NavBarProps) {
  // An override set to undefined (an i18n map with a missing key) keeps the default instead of replacing it.
  const labels = {
    ...defaultLabels,
    ...Object.fromEntries(
      Object.entries(labelOverrides ?? {}).filter(([, value]) => value !== undefined)
    )
  } as NavBarLabels
  const format = useMemo(() => numberFormat(locale), [locale])
  const renderLink = linkRenderer(linkComponent)
  const [drawerOpen, setDrawerOpen] = useState(false)
  const menuButtonRef = useRef<HTMLButtonElement>(null)
  const drawerId = useId()
  const drawerTitleId = useId()
  const count = unreadCount(notifications?.count)
  const navRef = useRef<HTMLElement>(null)
  const closeDrawer = useCallback(() => {
    setDrawerOpen(false)
    menuButtonRef.current?.focus()
  }, [])
  const closeDrawerOnDesktop = useCallback(() => {
    setDrawerOpen(false)
    navRef.current?.querySelector<HTMLElement>('a[href]')?.focus()
  }, [])

  const hasAccountOverride = accountControl !== null && accountControl !== undefined
  const secondaryNavigation = drawerOpen
    ? typeof mobileSecondaryNavigation === 'function'
      ? mobileSecondaryNavigation({ closeDrawer })
      : mobileSecondaryNavigation
    : null

  const renderSignIn = (onDone?: () => void) =>
    !signInAction ? null : signInAction.onSelect ? (
      <button
        type="button"
        className={styles.signIn}
        onClick={() => {
          onDone?.()
          signInAction.onSelect()
        }}
      >
        {labels.signIn}
      </button>
    ) : (
      renderLink({
        href: signInAction.href,
        className: styles.signIn,
        onClick: onDone,
        children: labels.signIn
      })
    )

  return (
    <nav
      ref={navRef}
      className={className ? `${styles.root} ${className}` : styles.root}
      aria-label={labels.navigation}
      data-surface={surface}
    >
      <div className={styles.start}>
        {renderLink({
          href: logoHref,
          className: styles.logo,
          'aria-label': labels.home,
          children: <GeneralDecentralandColor className={styles.logoMark} aria-hidden="true" />
        })}
        <ul className={styles.sections}>
          {sections.map((section, index) =>
            section.items ? (
              <SectionDropdown key={index} section={section} renderLink={renderLink} />
            ) : (
              <li key={index} className={styles.section}>
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
        {(account && balances) || (!drawerOpen && hasSlot(balanceAction)) ? (
          <div className={styles.barBalances}>
            {account && balances ? (
              <Balances balances={balances} labels={labels} format={format} />
            ) : null}
            {!drawerOpen && hasSlot(balanceAction) ? (
              <div className={styles.compositionControl}>{balanceAction}</div>
            ) : null}
          </div>
        ) : null}
        {account && notifications ? (
          <NotificationsButton count={count} labels={labels} onClick={notifications.onSelect} />
        ) : null}
        {hasAccountOverride ? (
          !drawerOpen && hasSlot(accountControl) ? (
            <div className={styles.barAccountControl}>{accountControl}</div>
          ) : null
        ) : account ? (
          <AccountMenu
            account={account}
            actions={accountActions}
            labels={labels}
            renderLink={renderLink}
          />
        ) : (
          renderSignIn()
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
          <GeneralMenu size={24} aria-hidden="true" />
        </button>
      </div>
      {drawerOpen ? (
        <Drawer
          id={drawerId}
          titleId={drawerTitleId}
          onClose={closeDrawer}
          onDesktop={closeDrawerOnDesktop}
          labels={labels}
        >
          <ul className={styles.drawerSections}>
            {sections.map((section, index) => (
              <li key={index}>
                {section.items ? (
                  <>
                    <p className={styles.drawerGroup}>{section.label}</p>
                    <ul className={styles.drawerItems}>
                      {section.items.map((item, itemIndex) => (
                        <li key={itemIndex}>
                          {renderLink({
                            href: item.href,
                            className: styles.drawerLink,
                            'aria-current': item.active ? 'page' : undefined,
                            onClick: closeDrawer,
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
                    onClick: closeDrawer,
                    children: section.label
                  })
                )}
              </li>
            ))}
          </ul>
          {hasSlot(secondaryNavigation) ? (
            <div className={styles.secondaryNavigation}>{secondaryNavigation}</div>
          ) : null}
          <div className={styles.drawerAccount}>
            {!hasAccountOverride && account ? (
              <div className={styles.drawerIdentity}>
                <Avatar account={account} />
                <span className={styles.accountName}>
                  {account.name || shortAddress(account.address)}
                </span>
              </div>
            ) : null}
            {account && balances ? (
              <Balances balances={balances} labels={labels} format={format} />
            ) : null}
            {hasSlot(balanceAction) ? (
              <div className={styles.compositionControl}>{balanceAction}</div>
            ) : null}
            {account && notifications ? (
              <button
                type="button"
                className={styles.menuItem}
                onClick={() => {
                  closeDrawer()
                  notifications.onSelect()
                }}
              >
                {count > 0 ? labels.unreadNotifications(count) : labels.notifications}
              </button>
            ) : null}
            {accountControl ??
              (account ? (
                accountActions.length > 0 ? (
                  <AccountActionList
                    actions={accountActions}
                    renderLink={renderLink}
                    onDone={closeDrawer}
                    className={styles.menuList}
                  />
                ) : null
              ) : (
                renderSignIn(closeDrawer)
              ))}
          </div>
        </Drawer>
      ) : null}
    </nav>
  )
}
