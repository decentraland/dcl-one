import type { ComponentType, ReactNode } from 'react'

import { Button, type PlaceCardLinkProps } from '../../../index'
import { CardImage } from '../../../components/CardImage/CardImage'
import { GeneralDecentralandColor, GeneralGallery } from '../../../foundations/icons'
import { EmptyMessage } from '../profile/EmptyMessage'
import styles from './ReelsPage.module.css'

export function ReelsPage({
  title,
  children,
  gallery = false,
  onHomeClick
}: {
  title: string
  children: ReactNode
  gallery?: boolean
  /** Runs before the Decentraland logo's link to the home page is followed. */
  onHomeClick?: () => void
}) {
  return (
    <main className={styles.page}>
      <header className={styles.header}>
        <Button
          href="/"
          variant="text"
          iconOnly
          aria-label="Decentraland home"
          onClick={onHomeClick}
        >
          <GeneralDecentralandColor size={28} aria-hidden="true" />
        </Button>
        <h1 className={styles.title}>{title}</h1>
      </header>
      <div className={gallery ? styles.gallery : styles.content}>{children}</div>
    </main>
  )
}

export function ReelsGallery({
  photos,
  total,
  linkComponent: Link,
  onPhotoClick
}: {
  photos: { id: string; href: string; label: string; thumbnail: string; image: string }[]
  total: number
  linkComponent: ComponentType<PlaceCardLinkProps>
  /** Runs, with the photo's id, before its link is followed. */
  onPhotoClick?: (id: string) => void
}) {
  return (
    <>
      {total > photos.length ? (
        <p>
          Showing {photos.length} of {total} public photos
        </p>
      ) : null}
      <ul className={styles.grid}>
        {photos.map((photo) => (
          <li key={photo.id}>
            <Link
              to={photo.href}
              className={styles.tile}
              aria-label={photo.label}
              onClick={() => onPhotoClick?.(photo.id)}
            >
              <CardImage src={photo.thumbnail} fallbackSrc={photo.image} alt="" />
            </Link>
          </li>
        ))}
      </ul>
    </>
  )
}

export function ReelsEmpty({
  reason,
  onRetry
}: {
  reason: 'photo' | 'gallery' | 'invalid-gallery' | 'unavailable'
  onRetry?: () => void
}) {
  const copy = {
    photo: [
      'Photo not found',
      'The photo you are trying to access does not exist or is no longer available.'
    ],
    gallery: ['No public photos', 'This gallery has no public photos yet.'],
    'invalid-gallery': [
      'Gallery not found',
      'Check that the link contains a valid wallet address.'
    ],
    unavailable: ['Reels is temporarily unavailable', 'Try loading this page again.']
  }[reason]

  return (
    <EmptyMessage
      icon={<GeneralGallery size={32} />}
      title={copy[0]!}
      subtitle={copy[1]!}
      action={
        onRetry ? (
          <Button variant="outline" onClick={onRetry}>
            Try again
          </Button>
        ) : null
      }
    />
  )
}
