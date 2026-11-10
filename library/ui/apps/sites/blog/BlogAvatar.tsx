// TODO(figma-ds): replace with the generated Avatar component when the DCL UI library lands
import { blogInitials } from './blogModel'
import styles from './BlogAvatar.module.css'

export type BlogAvatarProps = { name: string; size?: 'sm' | 'md' }

/** An author's initials on the brand gradient: decorative, the name is always shown beside it. */
export function BlogAvatar({ name, size = 'sm' }: BlogAvatarProps) {
  return (
    <span className={`${styles.avatar} ${styles[size]}`} aria-hidden="true">
      {size === 'md' ? blogInitials(name) : null}
    </span>
  )
}
