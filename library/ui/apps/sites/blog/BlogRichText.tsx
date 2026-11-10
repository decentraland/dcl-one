import { Fragment, type ReactNode } from 'react'
import { inAppLink, type BlogNavigate } from './blogLinks'
import { isSafeHyperlinkUri } from './links'
import { TweetEmbed } from './TweetEmbed'
import styles from './BlogRichText.module.css'

// A post body, rendered as the source blog renders it (decentraland/sites@cfdfa1a5,
// components/Blog/RichText with @contentful/rich-text-react-renderer's defaults), over the node set
// of blog plan R4-4. Hand-written: no rich-text dependency. Nodes outside the set render their
// children (text when they have any, nothing otherwise), as the library does; the app records them.

/** A Contentful rich-text node, as the CMS returns it. */
export type BlogRichTextNode = {
  nodeType: string
  data?: Record<string, unknown>
  content?: BlogRichTextNode[]
  value?: string
  marks?: { type: string }[]
}

export type BlogRichTextAsset = { url: string; width?: number; height?: number; title?: string }

export type BlogRichTextProps = {
  body: BlogRichTextNode | null
  /** Resolved assets by id, for embedded images. */
  assets: Readonly<Record<string, BlogRichTextAsset>>
  /**
   * The blog's own base URL (the source's BLOG_BASE_URL). Links under it are internal: they render
   * as the path alone, as the source's router link does (query and hash dropped).
   */
  blogBaseUrl: string
  /** Follows an internal link without a document load; plain clicks only. */
  onNavigate?: BlogNavigate
}

// The source's allowlists (RichText.renderers.tsx, RichText.helpers.ts).
const YOUTUBE_HOSTS = new Set(['www.youtube.com', 'youtube.com', 'youtu.be'])
const TWITTER_HOSTS = new Set(['www.twitter.com', 'twitter.com', 'www.x.com', 'x.com'])
const INSTAGRAM_HOSTS = new Set(['www.instagram.com', 'instagram.com'])
const LINKEDIN_HOSTS = new Set(['www.linkedin.com', 'linkedin.com'])
const YOUTUBE_ID = /^[\w-]{1,20}$/
const TWEET_ID = /^\d{1,20}$/

const parseUrl = (uri: string): URL | null => {
  try {
    return new URL(uri)
  } catch {
    return null
  }
}

/**
 * The URL, for an embed: http or https only. A host check alone is not enough: a URL such as
 * `javascript://linkedin.com/%0Aalert(1)` parses with an allowlisted hostname. (The source checks
 * the host only; the LinkedIn embed there passes such a URL to an iframe. Reported upstream.)
 */
const parseEmbedUrl = (uri: string): URL | null => {
  const url = parseUrl(uri)
  return url && (url.protocol === 'https:' || url.protocol === 'http:') ? url : null
}

function youTubeId(uri: string): string | null {
  const url = parseEmbedUrl(uri)
  if (!url || !YOUTUBE_HOSTS.has(url.hostname)) return null
  const id = url.hostname === 'youtu.be' ? url.pathname.slice(1) : (url.searchParams.get('v') ?? '')
  return YOUTUBE_ID.test(id) ? id : null
}

function tweetId(uri: string): string | null {
  const url = parseEmbedUrl(uri)
  if (!url || !TWITTER_HOSTS.has(url.hostname)) return null
  const segments = url.pathname.split('/').filter(Boolean)
  const status = segments.indexOf('status')
  const id = status >= 0 ? segments[status + 1] : undefined
  return id && TWEET_ID.test(id) ? id : null
}

/** A link whose text is its URL becomes the provider's embed, when the host is allowlisted. */
function embed(uri: string): ReactNode | null {
  const youtube = youTubeId(uri)
  if (youtube)
    return (
      <iframe
        className={styles.video}
        src={`https://www.youtube.com/embed/${youtube}`}
        title="YouTube video player"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
        allowFullScreen
        loading="lazy"
      />
    )
  const tweet = tweetId(uri)
  if (tweet) return <TweetEmbed tweetId={tweet} className={styles.tweet} />
  const url = parseEmbedUrl(uri)
  if (url && INSTAGRAM_HOSTS.has(url.hostname)) {
    const path = url.pathname.endsWith('/') ? url.pathname : `${url.pathname}/`
    return (
      <iframe
        className={styles.instagram}
        src={`https://www.instagram.com${path}embed/`}
        title={uri.includes('/reel/') ? 'Instagram Reel' : 'Instagram Post'}
        scrolling="no"
        allowFullScreen
        loading="lazy"
      />
    )
  }
  if (url && LINKEDIN_HOSTS.has(url.hostname))
    return (
      <iframe className={styles.linkedin} src={uri} title="Embedded Linkedin Post" loading="lazy" />
    )
  return null
}

type Context = Omit<BlogRichTextProps, 'body'>

function hyperlink(node: BlogRichTextNode, ctx: Context): ReactNode {
  const uri = typeof node.data?.uri === 'string' ? node.data.uri : ''
  // As source: a link shows its first text node's value only.
  const text = node.content?.[0]?.value
  if (text === uri) {
    const embedded = embed(uri)
    if (embedded) return embedded
  }
  if (ctx.blogBaseUrl.length > 0 && uri.startsWith(ctx.blogBaseUrl)) {
    const path = new URL(uri, 'https://blog.invalid').pathname
    return (
      <a className={styles.link} href={path} {...inAppLink(path, ctx.onNavigate)}>
        {text}
      </a>
    )
  }
  if (uri.startsWith('#'))
    return (
      <a className={styles.link} href={uri} target="_self">
        {text}
      </a>
    )
  if (!isSafeHyperlinkUri(uri)) return <>{text}</>
  return (
    <a className={styles.link} href={uri} target="_blank" rel="noopener noreferrer">
      {text}
    </a>
  )
}

function embeddedAsset(node: BlogRichTextNode, ctx: Context): ReactNode {
  const target = node.data?.target as
    { sys?: { id?: string }; fields?: { file?: { url?: string } } } | undefined
  const id = target?.sys?.id
  const asset = id ? ctx.assets[id] : undefined
  if (asset)
    return (
      <img
        className={styles.image}
        src={asset.url}
        // R4-4: the asset title when there is one (the source leaves every image without alt text).
        alt={asset.title ?? ''}
        width={asset.width}
        height={asset.height}
        loading="lazy"
        decoding="async"
      />
    )
  const url = target?.fields?.file?.url
  if (url)
    return (
      <img
        className={styles.image}
        src={url.startsWith('//') ? `https:${url}` : url}
        alt=""
        loading="lazy"
        decoding="async"
      />
    )
  return null
}

// The library's mark renderers; a mark outside them leaves the text as it is.
const MARKS: Record<string, (children: ReactNode) => ReactNode> = {
  bold: (children) => <b>{children}</b>,
  italic: (children) => <i>{children}</i>,
  underline: (children) => <u>{children}</u>,
  code: (children) => <code>{children}</code>
}

type Block = (children: ReactNode) => ReactNode
const BLOCKS: Record<string, Block> = {
  document: (children) => children,
  paragraph: (children) => <p className={styles.paragraph}>{children}</p>,
  'heading-1': (children) => <h1 className={styles.h1}>{children}</h1>,
  'heading-2': (children) => <h2 className={styles.h2}>{children}</h2>,
  'heading-3': (children) => <h3 className={styles.h3}>{children}</h3>,
  'heading-4': (children) => <h4 className={styles.h4}>{children}</h4>,
  'heading-5': (children) => <h5 className={styles.h5}>{children}</h5>,
  // As source: its Heading6 is styled(Heading5), so it renders an h5.
  'heading-6': (children) => <h5 className={styles.h6}>{children}</h5>,
  'unordered-list': (children) => <ul className={styles.list}>{children}</ul>,
  'ordered-list': (children) => <ol className={styles.list}>{children}</ol>,
  'list-item': (children) => <li>{children}</li>,
  blockquote: (children) => <blockquote className={styles.quote}>{children}</blockquote>,
  hr: () => <hr />,
  table: (children) => (
    <table>
      <tbody>{children}</tbody>
    </table>
  ),
  'table-row': (children) => <tr>{children}</tr>,
  'table-header-cell': (children) => <th>{children}</th>,
  'table-cell': (children) => <td>{children}</td>
}

function render(node: BlogRichTextNode, ctx: Context): ReactNode {
  if (node.nodeType === 'text') {
    return (node.marks ?? []).reduce<ReactNode>(
      (value, mark) => MARKS[mark.type]?.(value) ?? value,
      node.value ?? ''
    )
  }
  if (node.nodeType === 'hyperlink') return hyperlink(node, ctx)
  if (node.nodeType === 'embedded-asset-block') return embeddedAsset(node, ctx)
  const children = (node.content ?? []).map((child, index) => (
    <Fragment key={index}>{render(child, ctx)}</Fragment>
  ))
  const block = BLOCKS[node.nodeType]
  // Outside the supported set: the children, as the library does (the app records the node).
  return block ? block(children) : <>{children}</>
}

export function BlogRichText({ body, ...ctx }: BlogRichTextProps) {
  if (!body?.content) return null
  return <>{render(body, ctx)}</>
}
