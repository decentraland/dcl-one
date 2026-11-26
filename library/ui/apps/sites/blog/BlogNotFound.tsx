import { GeneralLeftArrow } from '../../../foundations/icons/general'
import { BlogLink } from './BlogRouting'
import { BlogShell } from './BlogShell'
import { BLOG_HOME } from './blogModel'
import styles from './BlogNotFound.module.css'

export type BlogNotFoundKind = 'post' | 'category' | 'author'

export type BlogNotFoundProps = { kind: BlogNotFoundKind }

const TITLES: Record<BlogNotFoundKind, string> = {
  post: 'Post not found',
  category: 'Category not found',
  author: 'Author not found'
}

function BlogMessage({ title, text, back }: { title: string; text: string; back: boolean }) {
  return (
    <BlogShell>
      <div className={styles.box}>
        <h1 className={styles.title}>{title}</h1>
        <p className={styles.text}>{text}</p>
        {back ? (
          <BlogLink className={styles.link} href={BLOG_HOME}>
            <GeneralLeftArrow size={20} aria-hidden="true" />
            Back to the Blog
          </BlogLink>
        ) : null}
      </div>
    </BlogShell>
  )
}

/** The page for a post, category or author the blog does not have (the route answers 404). */
export function BlogNotFound({ kind }: BlogNotFoundProps) {
  return <BlogMessage title={TITLES[kind]} text="We couldn’t find that page on the blog." back />
}

/** The page shown while the blog's content source cannot be reached (the route answers 503). */
export function BlogUnavailable() {
  return (
    <BlogMessage
      title="The blog is temporarily unavailable"
      text="We couldn’t load the blog right now. Please try again in a few minutes."
      back={false}
    />
  )
}
