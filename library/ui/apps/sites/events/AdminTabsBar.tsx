import ChromeLink from '../../../components/AppChrome/ChromeLink'
import { chromeLinkClick, useChromeNav } from '../../../components/AppChrome/chrome-nav'
import { Button } from '../../../components/Button/Button'
import { adminCopy } from './admin-copy'
import styles from './AdminTabsBar.module.css'

export type AdminTab = 'events' | 'pending' | 'users'

export const ADMIN_ROUTES = {
  events: '/events',
  pending: '/events/admin/pending-events',
  users: '/events/admin/users',
  create: '/events/new-event'
} as const

export type AdminTabsBarProps = {
  /** The page's tab; none on a page outside the three. */
  active?: AdminTab
  /** Pending Events: an account that may approve or edit any event. */
  showPending: boolean
  /** Users: an account that may manage the grants of others. */
  showUsers: boolean
}

/**
 * The event administrators' bar under the site header, as decentraland.org's: Events, Pending Events
 * and Users as uppercase tabs (the current one underlined) and an outlined "+ Create Event". The
 * caller renders it only for an account with an admin permission.
 */
export function AdminTabsBar({ active, showPending, showUsers }: AdminTabsBarProps) {
  const { navigate } = useChromeNav()
  const tabs = [
    { id: 'events' as const, label: adminCopy.tabs.events, href: ADMIN_ROUTES.events },
    ...(showPending
      ? [{ id: 'pending' as const, label: adminCopy.tabs.pending, href: ADMIN_ROUTES.pending }]
      : []),
    ...(showUsers
      ? [{ id: 'users' as const, label: adminCopy.tabs.users, href: ADMIN_ROUTES.users }]
      : [])
  ]
  return (
    <nav className={styles.root} aria-label={adminCopy.navigation}>
      <ul className={styles.tabs}>
        {tabs.map((tab) => (
          <li key={tab.id}>
            <ChromeLink
              className={styles.tab}
              href={tab.href}
              aria-current={tab.id === active ? 'page' : undefined}
            >
              {tab.label}
            </ChromeLink>
          </li>
        ))}
      </ul>
      <Button
        className={styles.create}
        variant="tertiary"
        size="md"
        href={ADMIN_ROUTES.create}
        onClick={(event) => chromeLinkClick(event, ADMIN_ROUTES.create, navigate)}
      >
        {adminCopy.createEvent}
      </Button>
    </nav>
  )
}
