import { readFileSync } from 'node:fs'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { type CreatorHubDownload, startCreatorHubDownload } from '~/lib/creatorHubDownload'
import { sendOverviewTrack as track } from '~/lib/overviewSegment'
import { heroData } from '../data'
import { Hero } from './Hero'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn(), sendOverviewPage: vi.fn() }))

const viewport = vi.hoisted(() => ({ mobile: false, reducedMotion: false }))
vi.mock('~/hooks/useMediaQuery', () => ({
  useMediaQuery: (query: string) => (query.includes('reduced-motion') ? viewport.reducedMotion : viewport.mobile)
}))

const release = vi.hoisted(() => ({
  download: undefined as CreatorHubDownload | undefined,
  others: [] as CreatorHubDownload[]
}))
vi.mock('~/hooks/useCreatorHubDownload', () => ({
  useCreatorHubDownload: () =>
    release.download ? { download: release.download, others: release.others } : { fallback: 'loading' }
}))
vi.mock('~/lib/creatorHubDownload', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/creatorHubDownload')>()),
  startCreatorHubDownload: vi.fn(),
  cancelCreatorHubRedirect: vi.fn(),
  macArchHint: () => null
}))

beforeEach(() => {
  viewport.mobile = false
  viewport.reducedMotion = false
  release.download = undefined
  release.others = []
  vi.mocked(startCreatorHubDownload).mockClear()
  vi.mocked(track).mockClear()
})

const renderHero = () =>
  render(
    <TranslationProvider>
      <Hero />
    </TranslationProvider>
  )

describe('Hero', () => {
  it('sends desktop visitors to the download page until an installer is known', () => {
    renderHero()
    const cta = screen.getByTestId('overview-hero-cta')
    expect(cta).toHaveTextContent('Download Creator Hub')
    expect(cta).toHaveAttribute('href', 'https://decentraland.zone/download/creator-hub')
    expect(cta).not.toHaveAttribute('target')

    fireEvent.click(cta)
    expect(startCreatorHubDownload).not.toHaveBeenCalled()
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Hero',
      event: 'Download',
      download_target: 'creator_hub',
      download_mode: 'loading',
      download_option: 'primary'
    })
  })

  it('downloads the installer for the visitor OS in one click', () => {
    release.download = { os: 'macOS', arch: 'arm64', href: 'https://example.com/creator-hub-mac-arm64.dmg' }
    renderHero()
    const cta = screen.getByTestId('overview-hero-cta')
    expect(cta).toHaveAttribute('href', 'https://example.com/creator-hub-mac-arm64.dmg')

    fireEvent.click(cta)
    expect(startCreatorHubDownload).toHaveBeenCalledWith(release.download, 'primary')
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Hero',
      event: 'Download',
      os: 'macOS',
      arch: 'arm64',
      download_target: 'creator_hub',
      download_mode: 'direct',
      download_option: 'primary'
    })
  })

  it("shows the installer's OS and offers the other OS's installer, in one click too", () => {
    release.download = { os: 'macOS', arch: 'arm64', href: 'https://example.com/creator-hub-mac-arm64.dmg' }
    const windows = { os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' } as const
    const intel = { os: 'macOS', arch: 'amd64', href: 'https://example.com/creator-hub-mac-x64.dmg' } as const
    release.others = [windows, intel]
    renderHero()
    expect(screen.getByTestId('overview-hero-os-icon')).toHaveAttribute('data-os', 'macOS')
    expect(screen.getByTestId('overview-hero-also-available')).toHaveTextContent('Also available on')
    expect(screen.getByRole('link', { name: 'Download for Intel-based Mac' })).toHaveTextContent('Intel')

    const alt = screen.getByRole('link', { name: 'Download for Windows' })
    expect(alt).toHaveAttribute('href', windows.href)
    fireEvent.click(alt)
    expect(startCreatorHubDownload).toHaveBeenCalledWith(windows, 'alternative')
    expect(track).toHaveBeenCalledWith('Click', {
      place: 'Creators Hero',
      event: 'Download',
      os: 'Windows',
      arch: 'amd64',
      download_target: 'creator_hub',
      download_mode: 'direct',
      download_option: 'alternative'
    })
  })

  it('keeps the button, and its focus, when the release resolves', () => {
    const { rerender } = renderHero()
    const cta = screen.getByTestId('overview-hero-cta')
    cta.focus()
    release.download = { os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' }
    rerender(
      <TranslationProvider>
        <Hero />
      </TranslationProvider>
    )
    expect(screen.getByTestId('overview-hero-cta')).toBe(cta)
    expect(cta).toHaveFocus()
  })

  it('names the build the button downloads on hover', async () => {
    release.download = { os: 'macOS', arch: 'arm64', href: 'https://example.com/creator-hub-mac-arm64.dmg' }
    renderHero()
    fireEvent.mouseOver(screen.getByTestId('overview-hero-cta'))
    expect(await screen.findByTestId('overview-hero-cta-tooltip')).toHaveTextContent(
      'Download for Mac with Apple silicon'
    )
    // The tooltip describes the button; its own label stays its name.
    expect(screen.getByRole('link', { name: 'Download Creator Hub' })).toHaveAccessibleDescription(
      'Download for Mac with Apple silicon'
    )
  })

  it('offers no other OS until an installer is known', () => {
    renderHero()
    expect(screen.queryByTestId('overview-hero-os-icon')).toBeNull()
    expect(screen.queryByTestId('overview-hero-also-available')).toBeNull()
  })

  it('drops a repeat click while the first download is still redirecting', () => {
    release.download = { os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' }
    vi.mocked(startCreatorHubDownload).mockReturnValueOnce(true).mockReturnValueOnce(false)
    renderHero()
    const cta = screen.getByTestId('overview-hero-cta')
    expect(fireEvent.click(cta)).toBe(true)
    expect(fireEvent.click(cta)).toBe(false)
  })

  it('leaves a modifier click (new tab) to the browser', () => {
    release.download = { os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' }
    renderHero()
    fireEvent.click(screen.getByTestId('overview-hero-cta'), { metaKey: true })
    expect(startCreatorHubDownload).not.toHaveBeenCalled()
  })

  it('sends phones to the creator docs instead of the desktop-only download', () => {
    viewport.mobile = true
    renderHero()
    const cta = screen.getByTestId('overview-hero-cta')
    expect(cta).toHaveTextContent('Explore the Creator Docs')
    expect(cta).toHaveAttribute('href', 'https://docs.decentraland.org/creator/')
    expect(cta).not.toHaveAttribute('target')

    fireEvent.click(cta)
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Hero' })
  })

  it('types the rotating word into the title', () => {
    renderHero()
    expect(screen.getByTestId('overview-hero-word')).toHaveTextContent('W')
  })

  it('scrolls down to the next section from the chevron and tracks it', () => {
    window.scrollBy = vi.fn()
    renderHero()
    fireEvent.click(screen.getByRole('button', { name: 'Scroll to the next section' }))
    expect(window.scrollBy).toHaveBeenCalled()
    expect(track).toHaveBeenCalledWith('Click', { place: 'Creators Hero', title: 'scroll-to-why' })
  })

  it('paints the poster first and fades the video in on desktop once the page is idle', async () => {
    renderHero()
    expect(screen.getByTestId('overview-hero-poster')).toHaveAttribute('src', heroData.poster.url)

    const video = await screen.findByTestId('overview-hero-video')
    expect(video).not.toHaveAttribute('data-playing')
    fireEvent.playing(video)
    expect(video).toHaveAttribute('data-playing')
  })

  it('fades the video in again after it was dropped for a phone width', async () => {
    const { rerender } = renderHero()
    fireEvent.playing(await screen.findByTestId('overview-hero-video'))

    viewport.mobile = true
    rerender(
      <TranslationProvider>
        <Hero />
      </TranslationProvider>
    )
    expect(screen.queryByTestId('overview-hero-video')).not.toBeInTheDocument()

    viewport.mobile = false
    rerender(
      <TranslationProvider>
        <Hero />
      </TranslationProvider>
    )
    expect(await screen.findByTestId('overview-hero-video')).not.toHaveAttribute('data-playing')
  })

  it('keeps phones on the static poster', async () => {
    viewport.mobile = true
    renderHero()
    expect(screen.getByTestId('overview-hero-poster')).toHaveAttribute('src', heroData.poster.url)
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(screen.queryByTestId('overview-hero-video')).not.toBeInTheDocument()
  })

  it('hides the poster instead of a broken image when it fails to load', () => {
    renderHero()
    fireEvent.error(screen.getByTestId('overview-hero-poster'))
    expect(screen.queryByTestId('overview-hero-poster')).not.toBeInTheDocument()
  })

  it('keeps visitors who prefer reduced motion on the static poster', async () => {
    viewport.reducedMotion = true
    renderHero()
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(screen.queryByTestId('overview-hero-video')).not.toBeInTheDocument()
  })

  it('preloads the same poster from index.html', () => {
    const html = readFileSync(`${process.cwd()}/index.html`, 'utf8')
    // Vite fills %BASE_URL% with the same base the bundle reads from import.meta.env.BASE_URL.
    const base = import.meta.env.BASE_URL
    const candidates = heroData.poster.srcSet.split(', ').map(candidate => `%BASE_URL%${candidate.slice(base.length)}`)
    expect(html).toContain(candidates.join(', '))
  })
})
