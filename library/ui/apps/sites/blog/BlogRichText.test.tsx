import { act, fireEvent, render, within } from '@testing-library/react'
import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BlogRichText, type BlogRichTextNode } from './BlogRichText'
import { isSafeHyperlinkUri } from './links'

const BASE = 'https://decentraland.org/blog'
const text = (value: string, marks: string[] = []): BlogRichTextNode => ({
  nodeType: 'text',
  value,
  marks: marks.map((type) => ({ type }))
})
const node = (
  nodeType: string,
  content: BlogRichTextNode[] = [],
  data: Record<string, unknown> = {}
): BlogRichTextNode => ({ nodeType, content, data })
const doc = (...content: BlogRichTextNode[]) => node('document', content)
const link = (uri: string, label: string = uri) => node('hyperlink', [text(label)], { uri })

function html(body: BlogRichTextNode, assets = {}, onNavigate?: (path: string) => void) {
  const { container } = render(
    <BlogRichText body={body} assets={assets} blogBaseUrl={BASE} onNavigate={onNavigate} />
  )
  return container
}

describe('BlogRichText', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it.each([
    ['paragraph', 'p'],
    ['heading-1', 'h1'],
    ['heading-2', 'h2'],
    ['heading-3', 'h3'],
    ['heading-4', 'h4'],
    ['heading-5', 'h5'],
    // As source: its Heading6 is styled(Heading5).
    ['heading-6', 'h5'],
    ['blockquote', 'blockquote']
  ])('renders a %s as <%s>', (type, tag) => {
    const root = html(doc(node(type, [text('Hello')])))
    expect(root.firstElementChild?.tagName.toLowerCase()).toBe(tag)
    expect(root.textContent).toBe('Hello')
  })

  it('renders lists and their items', () => {
    const root = html(
      doc(
        node('unordered-list', [node('list-item', [text('a')])]),
        node('ordered-list', [node('list-item', [text('b')])])
      )
    )
    expect(root.querySelector('ul > li')?.textContent).toBe('a')
    expect(root.querySelector('ol > li')?.textContent).toBe('b')
  })

  it('renders a horizontal rule and a table inside a tbody', () => {
    const root = html(
      doc(
        node('hr'),
        node('table', [
          node('table-row', [
            node('table-header-cell', [text('H')]),
            node('table-cell', [text('C')])
          ])
        ])
      )
    )
    expect(root.querySelector('hr')).not.toBeNull()
    expect(root.querySelector('table > tbody > tr > th')?.textContent).toBe('H')
    expect(root.querySelector('table > tbody > tr > td')?.textContent).toBe('C')
  })

  it.each([
    ['bold', 'b'],
    ['italic', 'i'],
    ['underline', 'u'],
    ['code', 'code']
  ])('renders the %s mark as <%s>', (mark, tag) => {
    expect(
      html(doc(node('paragraph', [text('x', [mark])]))).querySelector(`p > ${tag}`)?.textContent
    ).toBe('x')
  })

  it('nests marks in the order the CMS lists them, the first innermost', () => {
    expect(
      html(doc(node('paragraph', [text('x', ['bold', 'italic'])]))).querySelector('p > i > b')
        ?.textContent
    ).toBe('x')
  })

  it('leaves text with an unknown mark as plain text', () => {
    expect(
      html(doc(node('paragraph', [text('x', ['superscript'])]))).querySelector('p')?.innerHTML
    ).toBe('x')
  })

  describe('images', () => {
    const image = (id: string) => node('embedded-asset-block', [], { target: { sys: { id } } })
    it('uses the asset title as alt text, with its size, lazily', () => {
      const img = html(doc(image('a1')), {
        a1: { url: 'https://cms.example/a.png', width: 10, height: 20, title: 'A title' }
      }).querySelector('img')!
      expect(img.getAttribute('src')).toBe('https://cms.example/a.png')
      expect(img.getAttribute('alt')).toBe('A title')
      expect([
        img.getAttribute('width'),
        img.getAttribute('height'),
        img.getAttribute('loading'),
        img.getAttribute('decoding')
      ]).toEqual(['10', '20', 'lazy', 'async'])
    })
    it('is decorative (alt="") when the asset has no title', () => {
      expect(
        html(doc(image('a1')), { a1: { url: 'https://cms.example/a.png' } })
          .querySelector('img')
          ?.getAttribute('alt')
      ).toBe('')
    })
    it('renders nothing for an asset it cannot resolve', () => {
      expect(html(doc(image('missing'))).innerHTML).toBe('')
    })
  })

  describe('links', () => {
    it('opens an external link in a new tab without opener', () => {
      const a = html(
        doc(node('paragraph', [link('https://example.org/x', 'Example')]))
      ).querySelector('a')!
      expect([a.getAttribute('href'), a.getAttribute('target'), a.getAttribute('rel')]).toEqual([
        'https://example.org/x',
        '_blank',
        'noopener noreferrer'
      ])
    })

    it('renders a blog link as its path alone, dropping query and hash, and follows it in place', () => {
      const onNavigate = vi.fn()
      const a = html(
        doc(node('paragraph', [link(`${BASE}/announcements/post?x=1#top`, 'Post')])),
        {},
        onNavigate
      ).querySelector('a')!
      expect(a.getAttribute('href')).toBe('/blog/announcements/post')
      expect(a.hasAttribute('target')).toBe(false)
      fireEvent.click(a)
      expect(onNavigate).toHaveBeenCalledWith('/blog/announcements/post')
      onNavigate.mockClear()
      fireEvent.click(a, { metaKey: true })
      expect(onNavigate).not.toHaveBeenCalled()
    })

    it('keeps an #anchor as authored, in the same tab', () => {
      const a = html(doc(node('paragraph', [link('#section', 'Jump')]))).querySelector('a')!
      expect([a.getAttribute('href'), a.getAttribute('target')]).toEqual(['#section', '_self'])
    })

    it('shows only the first text of a link, as the source does', () => {
      const a = html(
        doc(
          node('paragraph', [
            node('hyperlink', [text('one'), text('two', ['bold'])], { uri: 'https://example.org' })
          ])
        )
      ).querySelector('a')!
      expect(a.innerHTML).toBe('one')
    })

    it.each([
      'javascript:alert(1)',
      'JaVaScRiPt:alert(1)',
      ' javascript:alert(1)',
      'java\tscript:alert(1)',
      'data:text/html,<script>alert(1)</script>',
      'vbscript:msgbox(1)',
      'decentraland://open',
      'decentraland.org/download',
      ''
    ])('renders %j as inert text', (uri) => {
      const root = html(doc(node('paragraph', [link(uri, 'label')])))
      expect(root.querySelector('a')).toBeNull()
      expect(root.textContent).toBe('label')
      expect(isSafeHyperlinkUri(uri)).toBe(false)
    })

    it('keeps surrounding whitespace in a safe URI as the source does (the URL parser trims it, the href keeps it)', () => {
      const a = html(
        doc(node('paragraph', [link(' https://example.org/x ', 'Spaced')]))
      ).querySelector('a')!
      expect(a.getAttribute('href')).toBe(' https://example.org/x ')
      expect(a.getAttribute('target')).toBe('_blank')
    })

    it.each(['mailto:hello@example.org', '/blog/path', 'http://example.org'])(
      'allows %s',
      (uri) => {
        expect(isSafeHyperlinkUri(uri)).toBe(true)
      }
    )
  })

  describe('embeds (a link whose text is its URL)', () => {
    it.each([
      [
        'https://www.youtube.com/watch?v=abc_DEF-123',
        'iframe[src="https://www.youtube.com/embed/abc_DEF-123"]'
      ],
      ['https://youtu.be/abc123', 'iframe[src="https://www.youtube.com/embed/abc123"]'],
      [
        'https://x.com/decentraland/status/1641516308037509120',
        '[data-tweet-id="1641516308037509120"]'
      ],
      [
        'https://www.instagram.com/reel/ABC/',
        'iframe[src="https://www.instagram.com/reel/ABC/embed/"][title="Instagram Reel"]'
      ],
      [
        'https://www.instagram.com/p/ABC',
        'iframe[src="https://www.instagram.com/p/ABC/embed/"][title="Instagram Post"]'
      ],
      [
        'https://www.linkedin.com/feed/update/urn:li:activity:1/',
        'iframe[title="Embedded Linkedin Post"]'
      ]
    ])('embeds %s', (uri, selector) => {
      expect(html(doc(node('paragraph', [link(uri)]))).querySelector(selector)).not.toBeNull()
    })

    it.each([
      'https://www.youtube.com.evil.example/watch?v=abc',
      'https://evil.example/youtube.com/watch?v=abc',
      'https://notyoutube.com/watch?v=abc',
      'https://x.com.evil.example/a/status/1',
      'https://www.instagram.com.evil.example/p/ABC',
      'https://linkedin.com.evil.example/feed',
      'https://www.youtube.com/watch?v=<script>'
    ])('does not embed the lookalike or invalid %s, which stays a plain link', (uri) => {
      const root = html(doc(node('paragraph', [link(uri)])))
      expect(root.querySelector('iframe, [data-tweet-id]')).toBeNull()
      expect(root.querySelector('a')?.getAttribute('href')).toBe(uri)
    })

    it.each([
      'javascript://linkedin.com/%0Aalert(1)',
      'javascript://www.linkedin.com/%0Aalert(1)',
      'javascript://www.youtube.com/%0Aalert(1)?v=abc',
      'javascript://x.com/a/status/1%0Aalert(1)',
      'javascript://www.instagram.com/p/ABC%0Aalert(1)',
      'data://linkedin.com/text/html,x'
    ])(
      'never embeds %s, whose host is allowlisted but whose scheme is not http(s), and renders it inert',
      (uri) => {
        const root = html(doc(node('paragraph', [link(uri)])))
        expect(root.querySelector('iframe, [data-tweet-id]')).toBeNull()
        expect(root.querySelector('a')).toBeNull()
        expect(root.textContent).toBe(uri)
      }
    )

    it('keeps an X post inside its paragraph when the server HTML is parsed (no block element in <p>)', () => {
      const body = doc(
        node('paragraph', [link('https://x.com/decentraland/status/1641516308037509120')])
      )
      const parsed = document.createElement('div')
      parsed.innerHTML = renderToString(<BlogRichText body={body} assets={{}} blogBaseUrl={BASE} />)
      expect(parsed.querySelector('p > [data-tweet-id]')).not.toBeNull()
      expect(parsed.querySelectorAll('p')).toHaveLength(1)
      expect(parsed.querySelector('p div')).toBeNull()
    })

    it("lets X's script fill the embed, then drops the loading text without touching what the script added", async () => {
      const createTweet = vi.fn(async (_id: string, element: HTMLElement) => {
        const widget = document.createElement('div')
        // The fake stands in for X's output, which has no role or name to query; a test id marks it.
        widget.dataset.testid = 'x-widget'
        element.appendChild(widget)
        return widget
      })
      vi.stubGlobal('twttr', { widgets: { createTweet } })
      const root = html(
        doc(node('paragraph', [link('https://x.com/decentraland/status/1641516308037509120')]))
      )
      expect(root.textContent).toContain('Loading tweet…')
      await act(async () => undefined)
      expect(createTweet).toHaveBeenCalledWith('1641516308037509120', expect.any(HTMLElement), {
        theme: 'dark'
      })
      const embed = root.querySelector<HTMLElement>('[data-tweet-id]')
      expect(within(embed!).getByTestId('x-widget')).toBeInTheDocument()
      expect(root.textContent).not.toContain('Loading tweet…')
    })

    it('links instead of embedding when the text differs from the URL', () => {
      const root = html(doc(node('paragraph', [link('https://youtu.be/abc123', 'Watch')])))
      expect(root.querySelector('iframe')).toBeNull()
      expect(root.querySelector('a')?.textContent).toBe('Watch')
    })
  })

  describe('nodes outside the supported set', () => {
    it('renders their text children', () => {
      const root = html(
        doc(
          node('paragraph', [
            node('asset-hyperlink', [text('the asset')], { target: { sys: { id: 'x' } } })
          ])
        )
      )
      expect(root.querySelector('p')?.innerHTML).toBe('the asset')
    })
    it('renders nothing for a contentless one, such as an embedded entry of an unknown type', () => {
      expect(
        html(
          doc(
            node('embedded-entry-block', [], {
              target: { sys: { id: 'e1', contentType: { sys: { id: 'unknownType' } } } }
            })
          )
        ).innerHTML
      ).toBe('')
    })
  })

  it('renders nothing without a body', () => {
    expect(html(null as unknown as BlogRichTextNode).innerHTML).toBe('')
  })
})
