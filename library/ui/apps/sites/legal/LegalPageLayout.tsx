import type { CSSProperties, MouseEvent, ReactNode } from 'react'
import { LegalIcon, type LegalIconName } from './LegalIcon'
import styles from './LegalPageLayout.module.css'

type LegalLink = {
  type: 'link'
  href: string
  target?: string
  rel?: string
  children: readonly LegalInline[]
}

type LegalInline = string | LegalLink | { type: 'strong' | 'em'; children: readonly LegalInline[] }

type LegalBlock =
  | { type: 'h2' | 'h3' | 'h4'; id?: string; content: readonly LegalInline[] }
  | { type: 'p'; content: readonly LegalInline[] }
  | { type: 'ul'; items: readonly (readonly LegalInline[])[] }

type LegalSection = { id: string; blocks: readonly LegalBlock[] }

type LegalTocEntry = { id: string; label: string; depth?: number }

type LegalSidebarEntry = { href: string; label: string; icon: LegalIconName; active: boolean }

type LegalPageLayoutProps = {
  /** The document's h1. */
  title: string
  toc: readonly LegalTocEntry[]
  intro: readonly LegalBlock[]
  sections: readonly LegalSection[]
  /** A call to action after the body, rendered as a link styled as a button. */
  cta?: { href: string; target?: string; rel?: string; label: string }
  /** The documents, in order; the active one is marked as the current page. */
  sidebar: readonly LegalSidebarEntry[]
  /** The sidebar navigation's accessible name. */
  sidebarLabel: string
  /** The language of the document text, when it differs from the page's. */
  lang?: string
  /** Follows a sidebar link without a document load; plain clicks only. */
  onNavigate?: (href: string) => void
}

function Inline({ node }: { node: LegalInline }): ReactNode {
  if (typeof node === 'string') return node
  const children = node.children.map((child, i) => <Inline key={i} node={child} />)
  if (node.type === 'link')
    return (
      <a className={styles.link} href={node.href} target={node.target} rel={node.rel}>
        {children}
      </a>
    )
  return node.type === 'strong' ? <strong>{children}</strong> : <em>{children}</em>
}

const inlines = (nodes: readonly LegalInline[]) =>
  nodes.map((node, i) => <Inline key={i} node={node} />)

function Block({ block }: { block: LegalBlock }) {
  switch (block.type) {
    case 'h2':
      return (
        <h2 className={styles.h2} id={block.id}>
          {inlines(block.content)}
        </h2>
      )
    case 'h3':
      return (
        <h3 className={styles.h3} id={block.id}>
          {inlines(block.content)}
        </h3>
      )
    case 'h4':
      return (
        <h4 className={styles.h4} id={block.id}>
          {inlines(block.content)}
        </h4>
      )
    case 'p':
      return <p className={styles.p}>{inlines(block.content)}</p>
    case 'ul':
      return (
        <ul className={styles.list}>
          {block.items.map((item, i) => (
            <li key={i} className={styles.listItem}>
              {inlines(item)}
            </li>
          ))}
        </ul>
      )
  }
}

/** A plain primary click that the page can follow itself, as source's sidebar does. */
const isPlainClick = (event: MouseEvent<HTMLAnchorElement>) =>
  event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey

/** The Sites legal document page: the documents sidebar, then the title, contents and text. */
function LegalPageLayout({
  title,
  toc,
  intro,
  sections,
  cta,
  sidebar,
  sidebarLabel,
  lang,
  onNavigate
}: LegalPageLayoutProps) {
  return (
    <div className={styles.page}>
      <div className={styles.grid} lang={lang}>
        <nav className={styles.sidebar} aria-label={sidebarLabel}>
          {sidebar.map((entry) => (
            <a
              key={entry.href}
              className={styles.sidebarLink}
              href={entry.href}
              aria-current={entry.active ? 'page' : undefined}
              onClick={(event) => {
                if (!onNavigate || !isPlainClick(event)) return
                event.preventDefault()
                onNavigate(entry.href)
              }}
            >
              <LegalIcon name={entry.icon} className={styles.sidebarIcon} />
              {entry.label}
            </a>
          ))}
        </nav>
        <article className={styles.content}>
          <h1 className={styles.title}>{title}</h1>
          {toc.length > 0 ? (
            <ul className={styles.toc}>
              {toc.map((entry) => (
                <li
                  key={entry.id}
                  style={
                    entry.depth
                      ? ({ '--legal-toc-depth': entry.depth } as CSSProperties)
                      : undefined
                  }
                >
                  <a href={`#${entry.id}`}>{entry.label}</a>
                </li>
              ))}
            </ul>
          ) : null}
          {intro.map((block, i) => (
            <Block key={i} block={block} />
          ))}
          {sections.map((section) => (
            <section key={section.id} id={section.id} className={styles.section}>
              {section.blocks.map((block, i) => (
                <Block key={i} block={block} />
              ))}
            </section>
          ))}
          {cta ? (
            <a className={styles.cta} href={cta.href} target={cta.target} rel={cta.rel}>
              {cta.label}
            </a>
          ) : null}
        </article>
      </div>
    </div>
  )
}

export { LegalPageLayout }
export type {
  LegalBlock,
  LegalInline,
  LegalLink,
  LegalPageLayoutProps,
  LegalSection,
  LegalSidebarEntry,
  LegalTocEntry
}
