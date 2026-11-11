import { BlogCover } from './BlogCover'
import { blogHighlightRuns } from './blogHighlight'
import type { BlogSearchSuggestion } from './blogModel'
import styles from './BlogSearchResult.module.css'

export type BlogSearchResultProps = {
  post: BlogSearchSuggestion
  /** The text typed: the words it starts are highlighted in the title and description. */
  query: string
}

function Highlighted({ text, query }: { text: string; query: string }) {
  return (
    <>
      {blogHighlightRuns(text, query).map((run, i) =>
        run.match ? (
          <mark key={i} className={styles.match}>
            {run.text}
          </mark>
        ) : (
          run.text
        )
      )}
    </>
  )
}

/**
 * One row of the search dropdown (Figma "ResultsBlogEntry"): the post's picture, title and
 * description. It is not interactive: the search bar's option around it handles selection.
 */
export function BlogSearchResult({ post, query }: BlogSearchResultProps) {
  return (
    <div className={styles.row}>
      <BlogCover className={styles.thumb} image={post.image} hue={post.hue} decorative />
      <div className={styles.text}>
        <span className={styles.title}>
          <Highlighted text={post.title} query={query} />
        </span>
        {post.description ? (
          <span className={styles.excerpt}>
            <Highlighted text={post.description} query={query} />
          </span>
        ) : null}
      </div>
    </div>
  )
}
