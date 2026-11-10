import { GeneralLeftArrow } from '../../../foundations/icons/general'
import { BlogLink } from './BlogRouting'
import { BlogShell } from './BlogShell'
import { BLOG_HOME } from './blogModel'
import styles from './BlogNotFound.module.css'

export type BlogNotFoundProps = { slug: string }

/** The page for a post slug that does not exist (the route answers 404). */
export function BlogNotFound({ slug }: BlogNotFoundProps) {
  return (
    <BlogShell>
      <div className={styles.box}>
        <h1 className={styles.title}>Post not found</h1>
        <p className={styles.text}>We couldn’t find a blog post at “{slug}”.</p>
        <BlogLink className={styles.link} href={BLOG_HOME}>
          <GeneralLeftArrow size={20} aria-hidden="true" />
          Back to the Blog
        </BlogLink>
      </div>
    </BlogShell>
  )
}
