import { Fragment, type ReactNode } from 'react'
import { blogHref } from './blogLinks'
import type { BlogImage, BlogRichTextNode } from './blogModel'
import { BlogLink } from './BlogRouting'
import styles from './BlogRichText.module.css'

// A post body from the CMS (Contentful rich text). The node and mark set, and the link rules, follow
// decentraland/sites (see NOTICE). A node outside the set renders its children, so its text is kept
// and nothing throws.

export type BlogRichTextProps = {
  body: BlogRichTextNode | null
  /** The pictures the body embeds, by CMS asset id. */
  assets: Readonly<Record<string, BlogImage>>
}

const HEADING = /^heading-([1-6])$/

type Context = { assets: BlogRichTextProps['assets']; shift: number }

const MARKS: Record<string, (children: ReactNode) => ReactNode> = {
  bold: (children) => <strong>{children}</strong>,
  italic: (children) => <em>{children}</em>,
  underline: (children) => <u>{children}</u>,
  code: (children) => <code className={styles.code}>{children}</code>
}

const BLOCKS: Record<string, (children: ReactNode) => ReactNode> = {
  document: (children) => children,
  paragraph: (children) => <p className={styles.p}>{children}</p>,
  'unordered-list': (children) => <ul className={styles.list}>{children}</ul>,
  'ordered-list': (children) => <ol className={styles.list}>{children}</ol>,
  'list-item': (children) => <li>{children}</li>,
  blockquote: (children) => <blockquote className={styles.quote}>{children}</blockquote>,
  table: (children) => (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <tbody>{children}</tbody>
      </table>
    </div>
  ),
  'table-row': (children) => <tr>{children}</tr>,
  'table-header-cell': (children) => <th>{children}</th>,
  'table-cell': (children) => <td>{children}</td>
}

function heading(level: number, children: ReactNode, ctx: Context): ReactNode {
  const shifted = level - ctx.shift
  const Tag = shifted <= 2 ? 'h2' : shifted === 3 ? 'h3' : 'h4'
  return <Tag className={styles[Tag]}>{children}</Tag>
}

function hyperlink(node: BlogRichTextNode, children: ReactNode): ReactNode {
  const target = blogHref(node.data?.uri)
  if (!target) return children
  if (target.kind === 'external')
    return (
      <a className={styles.link} href={target.href} target="_blank" rel="noopener noreferrer">
        {children}
      </a>
    )
  if (target.href.startsWith('#'))
    return (
      <a className={styles.link} href={target.href}>
        {children}
      </a>
    )
  return (
    <BlogLink className={styles.link} href={target.href}>
      {children}
    </BlogLink>
  )
}

function embeddedAsset(node: BlogRichTextNode, ctx: Context): ReactNode {
  const id = (node.data?.target as { sys?: { id?: unknown } } | undefined)?.sys?.id
  const asset = typeof id === 'string' ? ctx.assets[id] : undefined
  if (!asset) return null
  return (
    <img
      className={styles.image}
      src={asset.url}
      alt={asset.alt}
      width={asset.width || undefined}
      height={asset.height || undefined}
      loading="lazy"
      decoding="async"
    />
  )
}

function render(node: BlogRichTextNode, ctx: Context): ReactNode {
  if (node.nodeType === 'text')
    return (node.marks ?? []).reduce<ReactNode>(
      (value, mark) => MARKS[mark.type]?.(value) ?? value,
      node.value ?? ''
    )
  if (node.nodeType === 'hr') return <hr className={styles.rule} />
  if (node.nodeType === 'embedded-asset-block') return embeddedAsset(node, ctx)
  const children = (Array.isArray(node.content) ? node.content : []).map((child, index) => (
    <Fragment key={index}>{render(child, ctx)}</Fragment>
  ))
  if (node.nodeType === 'hyperlink') return hyperlink(node, children)
  const level = HEADING.exec(node.nodeType)
  if (level) return heading(Number(level[1]), children, ctx)
  const block = BLOCKS[node.nodeType]
  return block ? block(children) : children
}

/** The lowest heading level the body uses, so it renders as h2 under the page's h1. */
function headingShift(body: BlogRichTextNode): number {
  let min = 7
  const walk = (node: BlogRichTextNode) => {
    const level = HEADING.exec(node.nodeType)
    if (level) min = Math.min(min, Number(level[1]))
    if (Array.isArray(node.content)) node.content.forEach(walk)
  }
  walk(body)
  return min === 7 ? 0 : min - 2
}

/** A post body. Headings are shifted so the body's highest level renders as h2 under the page's h1. */
export function BlogRichText({ body, assets }: BlogRichTextProps) {
  if (!body) return null
  return <div className={styles.body}>{render(body, { assets, shift: headingShift(body) })}</div>
}
