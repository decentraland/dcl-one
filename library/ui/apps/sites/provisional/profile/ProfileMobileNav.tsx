// TODO(figma-ds): replace with the phone's section list, until the Figma-generated mobile menu from library/ui/components.
import type { ComponentType, ReactNode } from 'react'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import { Icon, type IconName } from './icons'
import { CopyAddress, truncateAddress } from './ProfileHeader'
import styles from './ProfileMobileNav.module.css'
import type { TabLinkProps } from './ProfileTabs'

type ProfileSection = { id: string; label: string; href: string; icon: IconName }

type ProfileMobileNavProps = {
  address: string
  name: string
  nameColor: string
  avatarUrl?: string
  sections: readonly ProfileSection[]
  /** The row under the name: the owner shares the profile, a visitor adds a friend. */
  cta: ReactNode
  onSelect?: (id: string) => void
  /** The owner's sign-out, at the foot of the list. */
  onLogout?: () => void
  LinkComponent?: ComponentType<TabLinkProps>
}

function AnchorLink({ to, ...props }: TabLinkProps) {
  return <a href={to} {...props} />
}

/** The phone's root screen (the profile site's ProfileMobileNav): who, one call to action, the sections. */
function ProfileMobileNav({
  address,
  name,
  nameColor,
  avatarUrl,
  sections,
  cta,
  onSelect,
  onLogout,
  LinkComponent = AnchorLink
}: ProfileMobileNavProps) {
  return (
    <div className={styles.navScreen}>
      <div className={styles.userBlock}>
        <ProfileAvatar imageUrl={avatarUrl} name={name} color={nameColor} />
        <div className={styles.userNameColumn}>
          <p className={styles.userName}>{name}</p>
          <div className={styles.userAddressRow}>
            <p className={styles.userAddressText}>{truncateAddress(address)}</p>
            <CopyAddress address={address} className={styles.addressCopyButton} />
          </div>
        </div>
      </div>
      <div className={styles.ctaRow}>{cta}</div>
      <div className={styles.divider} />
      <nav className={styles.tabList} aria-label="Profile sections">
        {sections.map((section) => (
          <LinkComponent
            key={section.id}
            to={section.href}
            className={styles.tabItem ?? ''}
            onClick={() => onSelect?.(section.id)}
          >
            <span className={styles.tabLeading}>
              <Icon name={section.icon} className={styles.iconSmall} />
            </span>
            <span className={styles.tabLabel}>{section.label}</span>
            <span className={styles.tabChevron}>
              <Icon name="ChevronRight" className={styles.iconSmall} />
            </span>
          </LinkComponent>
        ))}
      </nav>
      {onLogout ? (
        <button type="button" className={styles.logout} onClick={onLogout}>
          <span className={styles.tabLeading}>
            <Icon name="Logout" className={styles.iconSmall} />
          </span>
          <span className={styles.tabLabel}>Logout</span>
        </button>
      ) : null}
    </div>
  )
}

export { ProfileMobileNav }
export type { ProfileMobileNavProps, ProfileSection }
