import { safeCssUrl } from './BlogSearch'
import styles from './BlogListHeaders.module.css'

// The headers above a category's and an author's posts: source's CategoryHero
// (components/blog/CategoryHero) and the AuthorPage header (pages/blog/AuthorPage.tsx).

export function BlogCategoryHero({
  title,
  description,
  image
}: {
  title: string
  description: string
  image: string
}) {
  const url = safeCssUrl(image)
  return (
    <div
      className={styles.hero}
      style={url ? { backgroundImage: `var(--blog-hero-shade), url("${url}")` } : undefined}
    >
      <div className={styles.heroContent}>
        <h3 className={styles.heroTitle}>{title}</h3>
        <p className={styles.heroText}>{description}</p>
      </div>
    </div>
  )
}

export function BlogAuthorHeader({
  name,
  description,
  image
}: {
  name: string
  description: string
  image?: string
}) {
  return (
    <div className={styles.author}>
      {image ? <img className={styles.authorImage} src={image} alt={name} /> : null}
      <h5 className={styles.authorName}>{name}</h5>
      <p className={styles.authorText}>{description}</p>
    </div>
  )
}
