// TODO(figma-ds): replace with the Figma-generated scene card from library/ui/components.
import type { ComponentType, ReactNode } from 'react'
import { GeneralMapPinFilled } from '../../../../foundations/icons'
import { Button } from '../../shared/Button/Button'
import { ProfileAvatar } from '../ProfileAvatar/ProfileAvatar'
import { useBrokenImage } from '../useBrokenImage'
import styles from './SceneCard.module.css'

/** The app's router link, so opening the scene is a client navigation; a plain anchor otherwise. */
type SceneCardLinkProps = { to: string; className: string; children: ReactNode }

type SceneCardCreator = {
  name: string
  /** The creator's profile page. */
  href: string
  faceUrl?: string
  /** Background behind the initial when there is no face. */
  color: string
}

type SceneCardProps = {
  title: string
  imageUrl?: string
  /** The scene's own page; the whole card opens it. */
  href: string
  creator?: SceneCardCreator
  /** Coordinates or a world name, shown as a chip that also jumps in. */
  location?: string
  /** Where Jump In goes, and what to do as it starts (tracking). Without it the card has no Jump In. */
  jumpIn?: { href: string; label?: string; onJumpIn?: () => void }
  LinkComponent?: ComponentType<SceneCardLinkProps>
}

function AnchorLink({ to, ...props }: SceneCardLinkProps) {
  return <a href={to} {...props} />
}

/**
 * A scene's image, title, creator and location. On wide screens hovering or focusing it shrinks
 * the image and reveals Jump In; on narrow screens Jump In and the location chip are hidden, as in
 * the source, and the scene's page carries them.
 */
function SceneCard({
  title,
  imageUrl,
  href,
  creator,
  location,
  jumpIn,
  LinkComponent = AnchorLink
}: SceneCardProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl)

  return (
    <article className={styles.root}>
      <div className={styles.media}>
        {imageUrl && !broken ? (
          <img
            ref={ref}
            className={styles.image}
            src={imageUrl}
            alt=""
            loading="lazy"
            onError={onError}
          />
        ) : null}
      </div>
      <div className={styles.content}>
        <div className={styles.info}>
          <h3 className={styles.title}>
            {/* The title's link covers the card, so the whole card opens the scene. */}
            <LinkComponent to={href} className={styles.cardLink}>
              {title}
            </LinkComponent>
          </h3>
          <div className={styles.meta}>
            {creator ? (
              <span className={styles.creator}>
                <span aria-hidden="true" className={styles.face}>
                  <ProfileAvatar
                    imageUrl={creator.faceUrl}
                    name={creator.name}
                    color={creator.color}
                    size={32}
                  />
                </span>
                <span className={styles.by}>
                  by{' '}
                  <LinkComponent to={creator.href} className={styles.creatorLink}>
                    {creator.name}
                  </LinkComponent>
                </span>
              </span>
            ) : null}
            {location ? (
              jumpIn ? (
                <a
                  className={styles.location}
                  href={jumpIn.href}
                  onClick={jumpIn.onJumpIn}
                  aria-label={`Jump in to ${location}`}
                >
                  <GeneralMapPinFilled size={16} aria-hidden="true" />
                  <span className={styles.locationText}>{location}</span>
                </a>
              ) : (
                <span className={styles.location}>
                  <GeneralMapPinFilled size={16} aria-hidden="true" />
                  <span className={styles.locationText}>{location}</span>
                </span>
              )
            ) : null}
          </div>
        </div>
        {jumpIn ? (
          <div className={styles.jump}>
            <Button href={jumpIn.href} onClick={jumpIn.onJumpIn}>
              {jumpIn.label ?? 'Jump in'}
            </Button>
          </div>
        ) : null}
      </div>
    </article>
  )
}

export { SceneCard }
export type { SceneCardCreator, SceneCardLinkProps, SceneCardProps }
