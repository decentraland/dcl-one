import type { AnchorHTMLAttributes, ComponentType, MouseEvent } from 'react'
import { GeneralPrivate, GeneralPublic } from '../../foundations/icons/general'
import { CardImage } from '../CardImage/CardImage'
import { Skeleton } from '../Skeleton/Skeleton'
import styles from './CommunityCard.module.css'

/** The router's link, passed in so the card opens its community under any router. */
export type CommunityCardLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }
export type CommunityCardLinkComponent = ComponentType<CommunityCardLinkProps>

export type CommunityCardLabels = {
  public: string
  private: string
  /** The CTA's text, drawn upper case. */
  view: string
}

const DEFAULT_LABELS: CommunityCardLabels = { public: 'Public', private: 'Private', view: 'View' }

export type CommunityCardProps = {
  /** Figma "Loading": a skeleton in place of the card, marked busy. */
  loading?: boolean
  /** The name assistive technology reads for the loading card. */
  loadingLabel?: string
  name?: string
  /** The community's page. The whole card is the name link's target. */
  href?: string
  /** Runs on a click of the name link, before `href` is followed. */
  onOpen?: (event: MouseEvent<HTMLAnchorElement>) => void
  image?: string | null
  /** Loads the cover eagerly, for the first cards of a page. */
  priority?: boolean
  /** The owner's display name, under the community name. */
  ownerName?: string | null
  privacy?: 'public' | 'private'
  /** The member count already formatted for the viewer, e.g. "1,340 members"; nothing shows without it. */
  membersLabel?: string
  /** Shown when hover or keyboard focus shrinks the cover. */
  description?: string | null
  labels?: CommunityCardLabels
  /** Renders `href` (a router link); a plain anchor without it. */
  linkComponent?: CommunityCardLinkComponent
  /** The name's heading level. */
  headingLevel?: 2 | 3 | 4 | 5 | 6
}

function Anchor(props: CommunityCardLinkProps) {
  return <a {...props} />
}

/**
 * A community in a grid: its cover, name, owner, privacy and member count, and a VIEW call to action.
 * Keyboard focus inside the card, and hover on a device that hovers from 768px, shrinks the cover and
 * reveals the description. The name is the card's link (its hit area covers the card); VIEW repeats it visually.
 */
export function CommunityCard({
  loading = false,
  loadingLabel = 'Loading',
  name = '',
  href,
  onOpen,
  image,
  priority = false,
  ownerName,
  privacy = 'public',
  membersLabel,
  description,
  labels = DEFAULT_LABELS,
  linkComponent: Link = Anchor,
  headingLevel = 3
}: CommunityCardProps) {
  if (loading) {
    return (
      <div className={styles.container} role="progressbar" aria-label={loadingLabel} aria-busy="true">
        <div className={`${styles.card} ${styles.loading}`}>
          <Skeleton className={styles.skeletonCover} />
          <Skeleton className={styles.skeletonName} />
          <Skeleton className={styles.skeletonLine} />
        </div>
      </div>
    )
  }
  const Heading = `h${headingLevel}` as const
  const PrivacyIcon = privacy === 'private' ? GeneralPrivate : GeneralPublic
  const about = description?.trim()
  return (
    <div className={styles.container}>
      <article className={styles.card}>
        <div className={styles.media}>
          <CardImage className={styles.cover} src={image} alt="" priority={priority} />
        </div>
        <div className={styles.body}>
          <Heading className={styles.name}>
            {href ? (
              <Link className={styles.link} href={href} onClick={onOpen}>
                {name}
              </Link>
            ) : (
              name
            )}
          </Heading>
          {ownerName ? <p className={styles.owner}>{ownerName}</p> : null}
          <p className={styles.meta}>
            <span className={styles.privacy}>
              <PrivacyIcon size={12} aria-hidden="true" />
              {privacy === 'private' ? labels.private : labels.public}
            </span>
            {membersLabel ? <span className={styles.members}>{membersLabel}</span> : null}
          </p>
          {about ? <p className={styles.description}>{about}</p> : null}
          <span className={styles.cta} aria-hidden="true">
            {labels.view}
          </span>
        </div>
      </article>
    </div>
  )
}
