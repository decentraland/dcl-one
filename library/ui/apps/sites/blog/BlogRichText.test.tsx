import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
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
  let container: HTMLElement

  describe('when the body has paragraphs, quotes, lists, rules and marks', () => {
    beforeEach(() => {
      ;({ container } = renderBody(
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
      ))
    })

    it('should render each mark with its element', () => {
      expect(screen.getByText('bold').tagName).toBe('STRONG')
      expect(screen.getByText('it').tagName).toBe('EM')
      expect(screen.getByText('u').tagName).toBe('U')
      expect(screen.getByText('c').tagName).toBe('CODE')
    })

    it('should render the quote as a blockquote', () => {
      expect(screen.getByText('Said').closest('blockquote')).not.toBeNull()
    })

    it('should render the items of both lists in order', () => {
      expect(screen.getAllByRole('listitem').map((li) => li.textContent)).toEqual(['One', 'First'])
    })

    it('should render the ordered list as an ol', () => {
      expect(container.querySelector('ol')).not.toBeNull()
    })

    it('should render the rule as an hr', () => {
      expect(container.querySelector('hr')).not.toBeNull()
    })
  })

  describe('when the highest body heading is below heading-2', () => {
    beforeEach(() => {
      renderBody(doc(node('heading-3', [text('Top')]), node('heading-5', [text('Sub')])))
    })

    it('should shift the highest heading to h2', () => {
      expect(screen.getByRole('heading', { level: 2, name: 'Top' })).toBeInTheDocument()
    })

    it('should shift the lower headings by the same amount', () => {
      expect(screen.getByRole('heading', { level: 4, name: 'Sub' })).toBeInTheDocument()
    })
  })

  describe('when the body has a heading-1', () => {
    beforeEach(() => {
      renderBody(doc(node('heading-1', [text('A')]), node('heading-2', [text('B')])))
    })

    it('should never render a second h1', () => {
      expect(screen.queryByRole('heading', { level: 1 })).toBeNull()
    })

    it('should render it under the page h1, as h2', () => {
      expect(screen.getByRole('heading', { level: 2, name: 'A' })).toBeInTheDocument()
    })

    it('should shift the lower headings by the same amount', () => {
      expect(screen.getByRole('heading', { level: 3, name: 'B' })).toBeInTheDocument()
    })
  })

  describe('when the body has external, internal and anchor links', () => {
    beforeEach(() => {
      renderBody(
        doc(
          node('paragraph', [
            link('https://example.org/a', 'external'),
            link('/blog/platform/x', 'internal'),
            link('#faq', 'anchor')
          ])
        )
      )
    })

    it('should open external links in a new tab', () => {
      const external = screen.getByRole('link', { name: 'external' })
      expect(external).toHaveAttribute('target', '_blank')
      expect(external).toHaveAttribute('rel', 'noopener noreferrer')
    })

    it('should keep site paths in place', () => {
      expect(screen.getByRole('link', { name: 'internal' })).toHaveAttribute(
        'href',
        '/blog/platform/x'
      )
      expect(screen.getByRole('link', { name: 'internal' })).not.toHaveAttribute('target')
    })

    it('should keep anchors in place', () => {
      expect(screen.getByRole('link', { name: 'anchor' })).toHaveAttribute('href', '#faq')
    })
  })

  describe('when a link target is unsafe or missing', () => {
    beforeEach(() => {
      ;({ container } = renderBody(
        doc(
          node('paragraph', [
            link('javascript:alert(1)', 'js'),
            link('data:text/html,x', 'data'),
            link(undefined, 'none')
          ])
        )
      ))
    })

    it('should render no link', () => {
      expect(container.querySelector('a')).toBeNull()
    })

    it('should render the link text only', () => {
      expect(container.textContent).toBe('jsdatanone')
    })
  })

  describe('when the body embeds a known and an unknown asset', () => {
    beforeEach(() => {
      ;({ container } = renderBody(
        doc(
          node('embedded-asset-block', [], { target: { sys: { id: 'img' } } }),
          node('embedded-asset-block', [], { target: { sys: { id: 'gone' } } })
        ),
        { img: { url: 'https://images.example/a.png', width: 800, height: 400, alt: 'The map' } }
      ))
    })

    it('should render the known picture with its alt, size and lazy loading', () => {
      const img = screen.getByRole('img', { name: 'The map' })
      expect(img).toHaveAttribute('width', '800')
      expect(img).toHaveAttribute('loading', 'lazy')
    })

    it('should render nothing for the unknown asset', () => {
      expect(container.querySelectorAll('img')).toHaveLength(1)
    })
  })

  describe('when the body has unknown node types and odd data', () => {
    beforeEach(() => {
      ;({ container } = renderBody(
        doc(
          node('embedded-entry-block', [node('paragraph', [text('Inside a widget')])]),
          node('mystery', [text('Loose')]),
          { nodeType: 'paragraph', content: 'not an array' as never },
          { nodeType: 'text' },
          text('Bare', 'sparkle')
        )
      ))
    })

    it('should render their text without throwing', () => {
      expect(container.textContent).toBe('Inside a widgetLooseBare')
    })
  })

  describe('when the body has a table', () => {
    let unmount: () => void

    beforeEach(() => {
      ;({ container, unmount } = renderBody(
        doc(
          node('table', [
            node('table-row', [
              node('table-header-cell', [text('H')]),
              node('table-cell', [text('C')])
            ])
          ])
        )
      ))
    })

    it('should render its header cells', () => {
      expect(screen.getByRole('columnheader', { name: 'H' })).toBeInTheDocument()
    })

    it('should render its cells', () => {
      expect(screen.getByRole('cell', { name: 'C' })).toBeInTheDocument()
    })

    describe('and it is unmounted', () => {
      beforeEach(() => {
        unmount()
      })

      it('should leave nothing behind', () => {
        expect(container.textContent).toBe('')
      })
    })
  })

  describe('when the body is empty', () => {
    beforeEach(() => {
      ;({ container } = renderBody(null))
    })

    it('should render nothing', () => {
      expect(container.textContent).toBe('')
    })
  })
})
