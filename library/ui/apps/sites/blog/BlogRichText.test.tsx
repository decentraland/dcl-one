import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { BlogRichTextNode } from './blogModel'
import { BlogRichText } from './BlogRichText'

const text = (value: string, ...marks: string[]): BlogRichTextNode => ({
  nodeType: 'text',
  value,
  marks: marks.map((type) => ({ type }))
})
const node = (nodeType: string, content: BlogRichTextNode[] = [], data = {}): BlogRichTextNode => ({
  nodeType,
  content,
  data
})
const doc = (...content: BlogRichTextNode[]) => node('document', content)
const link = (uri: unknown, label: string) => node('hyperlink', [text(label)], { uri })

const renderBody = (body: BlogRichTextNode | null, assets = {}) =>
  render(<BlogRichText body={body} assets={assets} />)

describe('BlogRichText', () => {
  it('renders paragraphs, quotes, lists, rules and marks', () => {
    const { container } = renderBody(
      doc(
        node('paragraph', [
          text('Plain '),
          text('bold', 'bold'),
          text('it', 'italic'),
          text('u', 'underline'),
          text('c', 'code')
        ]),
        node('blockquote', [node('paragraph', [text('Said')])]),
        node('unordered-list', [node('list-item', [node('paragraph', [text('One')])])]),
        node('ordered-list', [node('list-item', [node('paragraph', [text('First')])])]),
        node('hr')
      )
    )
    expect(screen.getByText('bold').tagName).toBe('STRONG')
    expect(screen.getByText('it').tagName).toBe('EM')
    expect(screen.getByText('u').tagName).toBe('U')
    expect(screen.getByText('c').tagName).toBe('CODE')
    expect(screen.getByText('Said').closest('blockquote')).not.toBeNull()
    expect(screen.getAllByRole('listitem').map((li) => li.textContent)).toEqual(['One', 'First'])
    expect(container.querySelector('ol')).not.toBeNull()
    expect(container.querySelector('hr')).not.toBeNull()
  })

  it('shifts headings so the highest body level renders as h2', () => {
    renderBody(doc(node('heading-3', [text('Top')]), node('heading-5', [text('Sub')])))
    expect(screen.getByRole('heading', { level: 2, name: 'Top' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 4, name: 'Sub' })).toBeInTheDocument()
  })

  it('renders a heading-1 body under the page h1, never as a second h1', () => {
    renderBody(doc(node('heading-1', [text('A')]), node('heading-2', [text('B')])))
    expect(screen.queryByRole('heading', { level: 1 })).toBeNull()
    expect(screen.getByRole('heading', { level: 2, name: 'A' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 3, name: 'B' })).toBeInTheDocument()
  })

  it('opens external links in a new tab and keeps site paths and anchors in place', () => {
    renderBody(
      doc(
        node('paragraph', [
          link('https://example.org/a', 'external'),
          link('/blog/platform/x', 'internal'),
          link('#faq', 'anchor')
        ])
      )
    )
    const external = screen.getByRole('link', { name: 'external' })
    expect(external).toHaveAttribute('target', '_blank')
    expect(external).toHaveAttribute('rel', 'noopener noreferrer')
    expect(screen.getByRole('link', { name: 'internal' })).toHaveAttribute(
      'href',
      '/blog/platform/x'
    )
    expect(screen.getByRole('link', { name: 'internal' })).not.toHaveAttribute('target')
    expect(screen.getByRole('link', { name: 'anchor' })).toHaveAttribute('href', '#faq')
  })

  it('renders an unsafe or missing link target as its text only', () => {
    const { container } = renderBody(
      doc(
        node('paragraph', [
          link('javascript:alert(1)', 'js'),
          link('data:text/html,x', 'data'),
          link(undefined, 'none')
        ])
      )
    )
    expect(container.querySelector('a')).toBeNull()
    expect(container.textContent).toBe('jsdatanone')
  })

  it('renders an embedded picture with alt, size and lazy loading; nothing for an unknown asset', () => {
    const { container } = renderBody(
      doc(
        node('embedded-asset-block', [], { target: { sys: { id: 'img' } } }),
        node('embedded-asset-block', [], { target: { sys: { id: 'gone' } } })
      ),
      { img: { url: 'https://images.example/a.png', width: 800, height: 400, alt: 'The map' } }
    )
    const img = screen.getByRole('img', { name: 'The map' })
    expect(img).toHaveAttribute('width', '800')
    expect(img).toHaveAttribute('loading', 'lazy')
    expect(container.querySelectorAll('img')).toHaveLength(1)
  })

  it('renders the text of node types it does not know and never throws on odd data', () => {
    const { container } = renderBody(
      doc(
        node('embedded-entry-block', [node('paragraph', [text('Inside a widget')])]),
        node('mystery', [text('Loose')]),
        { nodeType: 'paragraph', content: 'not an array' as never },
        { nodeType: 'text' },
        text('Bare', 'sparkle')
      )
    )
    expect(container.textContent).toBe('Inside a widgetLooseBare')
  })

  it('renders tables and nothing for an empty body', () => {
    const { container, unmount } = renderBody(
      doc(
        node('table', [
          node('table-row', [
            node('table-header-cell', [text('H')]),
            node('table-cell', [text('C')])
          ])
        ])
      )
    )
    expect(screen.getByRole('columnheader', { name: 'H' })).toBeInTheDocument()
    expect(screen.getByRole('cell', { name: 'C' })).toBeInTheDocument()
    unmount()
    expect(container.textContent).toBe('')
    expect(renderBody(null).container.textContent).toBe('')
  })
})
