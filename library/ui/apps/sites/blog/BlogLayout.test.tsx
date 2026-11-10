import { fireEvent, render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { BlogLayout, BlogNavigation } from './BlogLayout'

const categories = [
  { id: 'c1', title: 'Announcements', url: '/blog/announcements' },
  { id: 'c2', title: 'Tutorials', url: '/blog/tutorials' }
]

describe('BlogNavigation', () => {
  it('lists All articles, then the given categories, inside a labelled nav', () => {
    const root = render(<BlogNavigation categories={categories} />).container
    expect(root.querySelector('nav')?.getAttribute('aria-label')).toBe('Blog')
    expect(
      [...root.querySelectorAll('nav li a')].map((a) => [a.getAttribute('href'), a.textContent])
    ).toEqual([
      ['/blog', 'All articles'],
      ['/blog/announcements', 'Announcements'],
      ['/blog/tutorials', 'Tutorials']
    ])
  })

  it.each([
    ['/blog', '/blog'],
    ['/blog/tutorials', '/blog/tutorials']
  ])('marks %s as the current page, and nothing else', (activeHref, marked) => {
    const root = render(
      <BlogNavigation categories={categories} activeHref={activeHref} />
    ).container
    expect(
      [...root.querySelectorAll('[aria-current]')].map((a) => [
        a.getAttribute('href'),
        a.getAttribute('aria-current')
      ])
    ).toEqual([[marked, 'page']])
  })

  it('marks nothing for a category outside the menu', () => {
    expect(
      render(
        <BlogNavigation categories={categories} activeHref="/blog/technology" />
      ).container.querySelector('[aria-current]')
    ).toBeNull()
  })

  it('puts the search box after the categories, and follows entries in the app', () => {
    const onNavigate = vi.fn()
    const root = render(
      <BlogNavigation
        categories={categories}
        search={<input aria-label="search" />}
        onNavigate={onNavigate}
      />
    ).container
    expect(root.querySelector('nav + input')).not.toBeNull()
    fireEvent.click(root.querySelector('a[href="/blog/tutorials"]')!)
    expect(onNavigate).toHaveBeenCalledWith('/blog/tutorials')
  })
})

describe('BlogLayout', () => {
  it('orders navigation, banner, content, then the after block', () => {
    const root = render(
      <BlogLayout navigation={<p>nav</p>} banner={<p>banner</p>} after={<p>after</p>}>
        <p>content</p>
      </BlogLayout>
    ).container
    expect(root.textContent).toBe('navbannercontentafter')
  })
})
