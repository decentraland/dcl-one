import type { ReactElement } from 'react'
import type { BlogBlock } from './blogModel'
import styles from './BlogRichText.module.css'

export type BlogRichTextProps = { blocks: readonly BlogBlock[] }

const LEVEL = { h2: 2, h3: 3, h4: 4 } as const

/**
 * A post body. Headings are shifted so the body's highest level renders as h2 under the page's
 * h1, keeping the outline free of skipped levels whatever level the data starts at.
 */
export function BlogRichText({ blocks }: BlogRichTextProps) {
  const levels = blocks.flatMap((b) =>
    b.type in LEVEL ? [LEVEL[b.type as keyof typeof LEVEL]] : []
  )
  const shift = levels.length ? Math.min(...levels) - 2 : 0
  return (
    <div className={styles.body}>
      {blocks.map((block, i): ReactElement => {
        switch (block.type) {
          case 'p':
            return (
              <p key={i} className={styles.p}>
                {block.content}
              </p>
            )
          case 'quote':
            return (
              <blockquote key={i} className={styles.quote}>
                <p>{block.content}</p>
              </blockquote>
            )
          case 'ul':
            return (
              <ul key={i} className={styles.list}>
                {block.items.map((item, j) => (
                  <li key={j}>{item}</li>
                ))}
              </ul>
            )
          default: {
            const level = LEVEL[block.type] - shift
            const Tag = level <= 2 ? 'h2' : level === 3 ? 'h3' : 'h4'
            return (
              <Tag key={i} className={styles[Tag]}>
                {block.content}
              </Tag>
            )
          }
        }
      })}
    </div>
  )
}
