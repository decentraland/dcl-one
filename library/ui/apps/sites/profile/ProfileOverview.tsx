import type { ReactNode } from 'react'

import { Button } from '../../../index'
import {
  GeneralEdit,
  GeneralLink,
  ProfileCountry,
  ProfileGender,
  ProfileHobbies,
  ProfileLanguage,
  ProfileProfession,
  ProfilePronouns,
  ProfileRealName,
  ProfileRelationship
} from '../../../foundations/icons'

import { ProfileBadges, type ProfileBadgeItem } from './ProfileBadges'
import { useBrokenImage } from '../useBrokenImage'
import styles from './ProfileOverview.module.css'

const INFO_ICONS: Record<string, ReactNode> = {
  country: <ProfileCountry size={20} />,
  language: <ProfileLanguage size={20} />,
  pronouns: <ProfilePronouns size={20} />,
  relationship_status: <ProfileRelationship size={20} />,
  gender: <ProfileGender size={20} />,
  profession: <ProfileProfession size={20} />,
  real_name: <ProfileRealName size={20} />,
  favorite_hobby: <ProfileHobbies size={20} />
}

/** A second safety net for any caller: only http(s) links are rendered. */
function isHttpUrl(url: string): boolean {
  try {
    const parsed = new URL(url)
    return parsed.protocol === 'http:' || parsed.protocol === 'https:'
  } catch {
    return false
  }
}

/** The avatar's full-body snapshot, as the source shows when its 3D preview cannot load. */
export function AvatarBody({ imageUrl, name }: { imageUrl?: string; name: string }) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)
  return (
    <div className={styles.body}>
      {imageUrl && !broken ? (
        <img
          ref={ref}
          className={styles.bodyImage}
          src={imageUrl}
          alt={`${name}'s avatar`}
          onError={onError}
        />
      ) : null}
    </div>
  )
}

export function EquippedItems({ children }: { children: ReactNode }) {
  return (
    <div className={styles.section}>
      <h2 className={styles.title}>Equipped items</h2>
      {children}
    </div>
  )
}

type ProfileOverviewProps = {
  own: boolean
  bio: string
  info: readonly { key: string; label: string; value: string }[]
  links: readonly { url: string; title: string }[]
  badges: readonly ProfileBadgeItem[]
  badgesLoading: boolean
  badgesError: boolean
  editHref: string
  equipped: ReactNode
}

export function ProfileOverview({
  own,
  bio,
  info,
  links: profileLinks,
  badges,
  badgesLoading,
  badgesError,
  editHref,
  equipped
}: ProfileOverviewProps) {
  const emptyBio = own
    ? 'No information yet. Jump in world and complete your bio!'
    : 'This member has not added a bio yet.'
  const links = profileLinks.filter((link) => isHttpUrl(link.url))
  return (
    <div className={styles.root}>
      <section className={styles.surface}>
        {own ? (
          <Button
            className={styles.edit}
            variant="dark"
            size="md"
            href={editHref}
            aria-label="Edit profile in Decentraland"
            trailingIcon={<GeneralEdit size={18} aria-hidden="true" />}
          >
            Edit
          </Button>
        ) : null}
        <div className={styles.section}>
          <ProfileBadges
            badges={badges}
            loading={badgesLoading}
            emptyText="No badges yet. Jump in world and start collecting badges!"
            // A failed background re-read keeps the badges already shown.
            errorText={badgesError ? 'Badges are unavailable right now.' : undefined}
          />
        </div>
        <div className={styles.section}>
          <h2 className={styles.title}>About</h2>
          {bio ? <p className={styles.bio}>{bio}</p> : <p className={styles.empty}>{emptyBio}</p>}
          {info.length > 0 ? (
            <dl className={styles.info}>
              {info.map((item) => {
                const icon = INFO_ICONS[item.key]
                return (
                  <div key={item.key} className={styles.infoItem}>
                    <dt className={styles.infoLabel}>
                      {icon ? <span className={styles.infoIcon}>{icon}</span> : null}
                      {item.label}
                    </dt>
                    <dd className={styles.infoValue}>{item.value}</dd>
                  </div>
                )
              })}
            </dl>
          ) : null}
        </div>
        {links.length > 0 ? (
          <div className={styles.section}>
            <h2 className={styles.title}>Links</h2>
            <ul className={styles.links}>
              {links.map((link, index) => (
                <li key={`${index}-${link.url}`}>
                  <a
                    className={styles.link}
                    href={link.url}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <GeneralLink size={22} aria-hidden="true" />
                    {link.title}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </section>
      <section className={styles.surface}>{equipped}</section>
    </div>
  )
}
