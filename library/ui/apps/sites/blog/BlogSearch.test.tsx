import { useState } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import {
  BlogSearch,
  safeCssUrl,
  searchPageHref,
  type BlogSearchHit,
  type BlogSearchProps
} from './BlogSearch'

const hit = (n: number): BlogSearchHit => ({
  id: `p${n}`,
  url: `/blog/announcements/p${n}`,
  image: `https://cms.example/p${n}.png`,
  highlightedTitle: `Post <em>${n}</em>`,
  highlightedDescription: `About ${n}`
})

type Props = Partial<Omit<BlogSearchProps, 'value' | 'onValueChange'>> & {
  initial?: string
  onNavigate?: (href: string) => void
}

function Harness({ initial = '', status = 'ready', hits = [], ...rest }: Props) {
  const [value, setValue] = useState(initial)
  return <BlogSearch value={value} onValueChange={setValue} status={status} hits={hits} {...rest} />
}

const input = () => screen.getByRole('combobox') as HTMLInputElement
const type = (value: string) => fireEvent.change(input(), { target: { value } })
const key = (name: string) => fireEvent.keyDown(input(), { key: name })

describe('BlogSearch', () => {
  it('opens its suggestions only once the value is over 2 characters (trimmed)', () => {
    render(<Harness hits={[hit(1)]} />)
    type(' ab ')
    expect(screen.queryByRole('listbox')).toBeNull()
    expect(input().getAttribute('aria-expanded')).toBe('false')
    type('abc')
    expect(screen.getByRole('listbox')).not.toBeNull()
    expect(input().getAttribute('aria-expanded')).toBe('true')
  })

  it.each([
    ['loading', 'Searching...'],
    ['error', 'Search errorSomething went wrong. Please try again.']
  ] as const)('shows the %s state', (status, text) => {
    render(<Harness initial="abc" status={status} hits={[hit(1)]} />)
    expect(screen.getByRole('listbox').textContent).toContain(text)
    expect(screen.queryAllByRole('option')).toHaveLength(0)
  })

  it('says there are no matches for an empty answer', () => {
    render(<Harness initial="abc" />)
    expect(screen.getByRole('listbox').textContent).toContain(
      'No matchesTry searching for something else'
    )
  })

  it('shows at most 4 hits with their highlights, then "See more results" when there are more', () => {
    render(<Harness initial="abc" hits={[1, 2, 3, 4, 5].map(hit)} />)
    const options = screen.getAllByRole('option')
    expect(options).toHaveLength(5)
    expect(options[0]!.querySelector('em')?.textContent).toBe('1')
    expect(options[4]!.querySelector('a')?.getAttribute('href')).toBe('/blog/search?q=abc')
  })

  it('shows no "See more results" with 4 hits or fewer', () => {
    render(<Harness initial="abc" hits={[1, 2, 3, 4].map(hit)} />)
    expect(screen.queryByText('See more results')).toBeNull()
  })

  it('opens the search page on Enter with nothing selected, and clears', () => {
    const onNavigate = vi.fn()
    render(<Harness initial="a b&c" hits={[hit(1)]} onNavigate={onNavigate} />)
    key('Enter')
    expect(onNavigate).toHaveBeenCalledWith('/blog/search?q=a%20b%26c')
    expect(input().value).toBe('')
  })

  it('moves the selection with the arrows, and opens the selected hit on Enter', () => {
    const onNavigate = vi.fn()
    render(<Harness initial="abc" hits={[1, 2].map(hit)} onNavigate={onNavigate} />)
    key('ArrowDown')
    key('ArrowDown')
    expect(input().getAttribute('aria-activedescendant')).toBe(screen.getAllByRole('option')[1]!.id)
    key('Enter')
    expect(onNavigate).toHaveBeenCalledWith('/blog/announcements/p2')
  })

  it('cycles as source does: with more than 3 hits the arrows reach the "more" entry (index 4)', () => {
    const onNavigate = vi.fn()
    render(<Harness initial="abc" hits={[1, 2, 3, 4, 5].map(hit)} onNavigate={onNavigate} />)
    key('ArrowUp')
    expect(screen.getAllByRole('option')[4]!.getAttribute('aria-selected')).toBe('true')
    key('ArrowDown')
    expect(screen.getAllByRole('option')[0]!.getAttribute('aria-selected')).toBe('true')
    key('ArrowUp')
    key('Enter')
    expect(onNavigate).toHaveBeenCalledWith('/blog/search?q=abc')
  })

  it('clears on Escape and with the clear button', () => {
    render(<Harness initial="abc" hits={[hit(1)]} />)
    key('Escape')
    expect(input().value).toBe('')
    type('abcd')
    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }))
    expect(input().value).toBe('')
  })

  it('closes when a hit is followed', () => {
    const onNavigate = vi.fn()
    render(<Harness initial="abc" hits={[hit(1)]} onNavigate={onNavigate} />)
    fireEvent.click(screen.getAllByRole('option')[0]!.querySelector('a')!)
    expect(onNavigate).toHaveBeenCalledWith('/blog/announcements/p1')
    expect(input().value).toBe('')
  })

  it('shows a hit image only from a safe https URL', () => {
    render(<Harness initial="abc" hits={[{ ...hit(1), image: 'javascript:alert(1)' }, hit(2)]} />)
    const images = screen
      .getAllByRole('option')
      .map((option) => (option.querySelector('a > div') as HTMLElement).style.backgroundImage)
    expect(images[0]).toBe('')
    expect(images[1]).toContain('https://cms.example/p2.png')
  })
})

describe('safeCssUrl', () => {
  it.each([
    ['https://cms.example/a.png', 'https://cms.example/a.png'],
    ['https://cms.example/a".png', 'https://cms.example/a%22.png'],
    ['http://cms.example/a.png', ''],
    ['javascript:alert(1)', ''],
    ['not a url', '']
  ])('%s gives %j', (url, expected) => {
    expect(safeCssUrl(url)).toBe(expected)
  })
})

describe('searchPageHref', () => {
  it('encodes the query', () => {
    expect(searchPageHref('a b&c')).toBe('/blog/search?q=a%20b%26c')
  })
})
