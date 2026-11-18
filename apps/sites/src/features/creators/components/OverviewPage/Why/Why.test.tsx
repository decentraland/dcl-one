import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { useLocale } from '~/store/locale'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { Why } from './Why'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: true }) }))

beforeEach(() => {
  useLocale.setState({ locale: 'en' })
  vi.mocked(track).mockClear()
})

describe('Why', () => {
  it('renders the three reasons as links out to Discord and the docs', () => {
    render(
      <TranslationProvider>
        <Why />
      </TranslationProvider>
    )
    const cards = screen.getAllByTestId('overview-why-card')
    expect(cards.map(card => card.getAttribute('href'))).toEqual([
      'https://dcl.gg/discord',
      'https://docs.decentraland.org/creator/',
      'https://docs.decentraland.org/creator/wearables-and-emotes/wearables/creating-wearables'
    ])
    expect(cards[0]).toHaveTextContent('Join the Discord')
    const heading = screen.getByRole('heading', { level: 2 })
    expect(heading).toHaveTextContent(/^Why creators choose Decentraland$/)
    expect(heading.querySelector('span')).toHaveTextContent(/^Why$/)
    expect(track).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Why', mobile: false })
  })

  it('tracks a card click under the card title', () => {
    render(
      <TranslationProvider>
        <Why />
      </TranslationProvider>
    )
    fireEvent.click(screen.getAllByTestId('overview-why-card')[0])
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Why', title: 'Join a Community of Creators' })
  })

  it('tracks the English card title for a Spanish-speaking visitor', () => {
    useLocale.setState({ locale: 'es' })
    render(
      <TranslationProvider>
        <Why />
      </TranslationProvider>
    )
    const card = screen.getAllByTestId('overview-why-card')[0]
    expect(card).toHaveTextContent('Únete a una comunidad de creadores')
    fireEvent.click(card)
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Why', title: 'Join a Community of Creators' })
  })

  it('renders the Chinese heading with no space after the highlight and marks the page as zh-Hans', () => {
    useLocale.setState({ locale: 'zh' })
    render(
      <TranslationProvider>
        <Why />
      </TranslationProvider>
    )
    expect(screen.getByRole('heading', { level: 2 })).toHaveTextContent(/^为什么创作者选择 Decentraland$/)
    expect(document.documentElement.lang).toBe('zh-Hans')
  })
})
