import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { useLocale } from '~/store/locale'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { Faqs } from './Faqs'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

const viewport = vi.hoisted(() => ({ inView: false }))
vi.mock('react-intersection-observer', () => ({ useInView: () => ({ ref: vi.fn(), inView: viewport.inView }) }))

beforeEach(() => {
  viewport.inView = false
  useLocale.setState({ locale: 'en' })
  vi.mocked(track).mockClear()
})

const renderSection = () =>
  render(
    <TranslationProvider>
      <Faqs />
    </TranslationProvider>
  )

describe('Faqs', () => {
  it('expands one question at a time and tracks only the expand', () => {
    renderSection()
    const [first, second] = screen.getAllByTestId('overview-faq')
    expect(first).toHaveAttribute('aria-expanded', 'false')

    fireEvent.click(first)
    expect(first).toHaveAttribute('aria-expanded', 'true')
    expect(first).toHaveAttribute('data-open')
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Faqs', title: 'What is Decentraland?' })

    fireEvent.click(second)
    expect(first).toHaveAttribute('aria-expanded', 'false')
    expect(second).toHaveAttribute('aria-expanded', 'true')

    fireEvent.click(second)
    expect(second).toHaveAttribute('aria-expanded', 'false')
    expect(track).toHaveBeenCalledTimes(2)
  })

  it('tracks the English question for a Spanish-speaking visitor', () => {
    useLocale.setState({ locale: 'es' })
    renderSection()
    const first = screen.getAllByTestId('overview-faq')[0]
    expect(first).toHaveTextContent('¿Qué es Decentraland?')
    fireEvent.click(first)
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Faqs', title: 'What is Decentraland?' })
  })

  it('toggles from the keyboard', () => {
    renderSection()
    const first = screen.getAllByTestId('overview-faq')[0]
    fireEvent.keyDown(first, { key: 'Enter' })
    expect(first).toHaveAttribute('aria-expanded', 'true')
    fireEvent.keyDown(first, { key: ' ' })
    expect(first).toHaveAttribute('aria-expanded', 'false')
    expect(track).toHaveBeenCalledTimes(1)
  })

  it('links to the docs FAQs and reports the section once it scrolls into view', () => {
    viewport.inView = true
    renderSection()
    const cta = screen.getByTestId('overview-faqs-cta')
    expect(cta).toHaveAttribute('href', 'https://docs.decentraland.org/faqs/')
    expect(cta).not.toHaveAttribute('target')
    fireEvent.click(cta)
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Faqs', title: 'faqs-cta' })
    expect(track).toHaveBeenCalledWith('Section Viewed', { section_viewed: 'Creators Faqs', mobile: false })
  })

  it("describes every question and answer to search engines in the visitor's language", () => {
    useLocale.setState({ locale: 'es' })
    renderSection()
    const data = JSON.parse(screen.getByTestId('overview-faqs-structured-data').textContent ?? '')
    expect(data['@type']).toBe('FAQPage')
    expect(data.mainEntity).toHaveLength(screen.getAllByTestId('overview-faq').length)
    expect(data.mainEntity[0]).toMatchObject({
      '@type': 'Question',
      name: '¿Qué es Decentraland?',
      acceptedAnswer: { '@type': 'Answer' }
    })
    expect(data.mainEntity[0].acceptedAnswer.text).not.toBe('')
  })
})
