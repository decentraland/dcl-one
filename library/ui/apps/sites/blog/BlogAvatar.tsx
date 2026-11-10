// TODO(figma-ds): replace with the generated Avatar component when the DCL UI library lands
import { blogInitials } from './blogModel'
import styles from './BlogAvatar.module.css'

export type BlogAvatarProps = { name: string; image?: string; size?: 'sm' | 'md' }

/** An author's picture, else initials on the brand gradient: decorative, the name is always shown beside it. */
export function BlogAvatar({ name, image, size = 'sm' }: BlogAvatarProps) {
  return (
    <span className={`${styles.avatar} ${styles[size]}`} aria-hidden="true">
      {image ? (
        <img className={styles.img} src={image} alt="" loading="lazy" decoding="async" />
      ) : size === 'md' ? (
        blogInitials(name)
      ) : null}
    </span>
  )
}
