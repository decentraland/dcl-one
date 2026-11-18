import { afterEach, describe, it, expect, beforeEach, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { TranslationProvider } from '~/intl'
import { sendOverviewTrack } from '~/lib/overviewSegment'
import { useLocale } from '~/store/locale'
import { Footer } from './Footer'

const viewport = vi.hoisted(() => ({ near: true }))
vi.mock('react-intersection-observer', () => ({
  useInView: ({ skip }: { skip?: boolean }) => ({ ref: vi.fn(), inView: !skip && viewport.near })
}))

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

const setReadyState = (state: DocumentReadyState) =>
  Object.defineProperty(document, 'readyState', { configurable: true, get: () => state })

const footerAt = (path: string) => (
  <TranslationProvider>
    <MemoryRouter initialEntries={[path]}>
      <Footer />
    </MemoryRouter>
  </TranslationProvider>
)

function renderFooter(path = '/collections') {
  return render(footerAt(path))
}

afterEach(() => {
  setReadyState('complete')
})

beforeEach(() => {
  viewport.near = true
  localStorage.clear()
  vi.mocked(sendOverviewTrack).mockClear()
  useLocale.setState({ locale: 'en' })
})

describe('Footer', () => {
  it("reports overview link and social clicks the way sites' landing footer did, in English", () => {
    useLocale.setState({ locale: 'es' })
    renderFooter('/')
    fireEvent.click(screen.getAllByTestId('footer-link')[0])
    fireEvent.click(screen.getAllByTestId('footer-social-link')[0])
    expect(sendOverviewTrack).toHaveBeenCalledWith('Click', {
      place: 'Landing Footer Link',
      event: 'click',
      link: 'What is Decentraland'
    })
    expect(sendOverviewTrack).toHaveBeenCalledWith('Click', {
      place: 'Landing Footer Social',
      event: 'click',
      platform: 'Discord'
    })
  })

  it('tracks nothing outside the overview', () => {
    renderFooter('/collections')
    fireEvent.click(screen.getAllByTestId('footer-link')[0])
    expect(sendOverviewTrack).not.toHaveBeenCalled()
  })

  it('switches the site language from the language menu', async () => {
    renderFooter()
    await userEvent.click(screen.getByRole('button', { name: 'English' }))
    await userEvent.click(screen.getByRole('button', { name: 'Español' }))

    // Copy re-renders in Spanish and the choice survives a reload.
    expect(screen.getAllByText('Recursos').length).toBeGreaterThan(0)
    expect(localStorage.getItem('creators:locale')).toBe('es')
  })

  it('closes the language menu with Escape and returns focus to the trigger', async () => {
    renderFooter()
    const trigger = screen.getByRole('button', { name: 'English' })
    await userEvent.click(trigger)
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: 'Español' })).toBeInTheDocument()

    await userEvent.keyboard('{Escape}')
    expect(screen.queryByRole('button', { name: 'Español' })).not.toBeInTheDocument()
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
  })

  // The accordion is mobile-only (hidden by a media query jsdom doesn't evaluate),
  // so it's selected by testid and clicked with fireEvent.
  it('expands and collapses the mobile menu sections', () => {
    renderFooter()
    const section = screen.getByTestId('footer-section-toggle-getting-started')
    expect(section).toHaveAttribute('aria-expanded', 'false')

    fireEvent.click(section)
    expect(section).toHaveAttribute('aria-expanded', 'true')
    const panel = document.getElementById(section.getAttribute('aria-controls')!)
    expect(panel).toHaveAttribute('data-open')

    fireEvent.click(section)
    expect(section).toHaveAttribute('aria-expanded', 'false')
    expect(panel).not.toHaveAttribute('data-open')
  })

  it('keeps the newsletter embed from navigating the page', () => {
    renderFooter()
    const frame = screen.getByTestId('footer-newsletter-frame')
    expect(frame).toHaveAttribute('sandbox')
    expect(frame.getAttribute('sandbox')).not.toContain('allow-top-navigation')
  })

  it('loads the newsletter embed only once the visitor nears the footer', async () => {
    viewport.near = false
    const { rerender } = renderFooter()
    const frame = screen.getByTestId('footer-newsletter-frame')
    expect(frame).not.toHaveAttribute('src')

    viewport.near = true
    rerender(footerAt('/collections'))
    await waitFor(() => expect(frame).toHaveAttribute('src', expect.stringContaining('embeds.beehiiv.com')))
  })

  it('holds the newsletter embed back until the page has finished loading', async () => {
    setReadyState('loading')
    renderFooter()
    const frame = screen.getByTestId('footer-newsletter-frame')
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(frame).not.toHaveAttribute('src')

    window.dispatchEvent(new Event('load'))
    await waitFor(() => expect(frame).toHaveAttribute('src', expect.stringContaining('embeds.beehiiv.com')))
  })
})
