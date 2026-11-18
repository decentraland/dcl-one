import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { sendOverviewTrack } from '~/lib/overviewSegment'
import { HttpError } from '~/lib/http'
import {
  cancelCreatorHubRedirect,
  fetchCreatorHubAssets,
  macArchFromRenderer,
  otherCreatorHubDownloads,
  pickCreatorHubDownload,
  startCreatorHubDownload,
  SUCCESS_REDIRECT_DELAY_MS,
  type Device,
  type MacArch
} from './creatorHubDownload'

vi.mock('~/lib/overviewSegment', () => ({ sendOverviewTrack: vi.fn() }))
vi.mock('~/lib/analytics', () => ({ ensureAnonymousId: () => 'anon-1' }))

const URL_BASE = 'https://github.com/decentraland/creator-hub/releases/download/0.50.0'
const asset = (name: string) => ({ name, browser_download_url: `${URL_BASE}/${name}` })
const assets = [
  asset('creator-hub-win-x64.exe'),
  asset('creator-hub-win-x64.exe.blockmap'),
  asset('creator-hub-mac-arm64.dmg'),
  asset('creator-hub-mac-x64.dmg')
]
const WINDOWS_UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
const MAC_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
const device = (userAgent: string, macArch: MacArch = 'unknown', maxTouchPoints = 0): Device => ({
  userAgent,
  maxTouchPoints,
  macArch: () => macArch
})

describe('fetchCreatorHubAssets', () => {
  afterEach(() => vi.unstubAllGlobals())

  it("reads the latest release's assets", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve({ assets }) })
    vi.stubGlobal('fetch', fetchMock)
    await expect(fetchCreatorHubAssets()).resolves.toEqual(assets)
    expect(fetchMock).toHaveBeenCalledWith(
      'https://api.github.com/repos/decentraland/creator-hub/releases/latest',
      expect.anything()
    )
  })

  it('treats a release without assets as no installers', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve({}) }))
    await expect(fetchCreatorHubAssets()).resolves.toEqual([])
  })

  it('treats the rate limit as no installers, so the CTA falls back without an error', async () => {
    const cancel = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 403, body: { cancel } }))
    await expect(fetchCreatorHubAssets()).resolves.toEqual([])
    expect(cancel).toHaveBeenCalled()
  })

  it('fails on a non-ok response, releasing its body', async () => {
    const cancel = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, status: 500, body: { cancel } }))
    await expect(fetchCreatorHubAssets()).rejects.toBeInstanceOf(HttpError)
    expect(cancel).toHaveBeenCalled()
  })
})

describe('pickCreatorHubDownload', () => {
  it('picks the Windows installer on Windows', () => {
    expect(pickCreatorHubDownload(assets, device(WINDOWS_UA))).toEqual({
      os: 'Windows',
      arch: 'amd64',
      href: `${URL_BASE}/creator-hub-win-x64.exe`
    })
  })

  it("picks the Mac build that matches the Mac's chip, Apple Silicon when it is unknown", () => {
    expect(pickCreatorHubDownload(assets, device(MAC_UA, 'apple_silicon'))).toEqual({
      os: 'macOS',
      arch: 'arm64',
      href: `${URL_BASE}/creator-hub-mac-arm64.dmg`,
      macArch: 'apple_silicon'
    })
    expect(pickCreatorHubDownload(assets, device(MAC_UA, 'intel'))).toEqual({
      os: 'macOS',
      arch: 'amd64',
      href: `${URL_BASE}/creator-hub-mac-x64.dmg`,
      macArch: 'intel'
    })
    expect(pickCreatorHubDownload(assets, device(MAC_UA))?.arch).toBe('arm64')
    expect(pickCreatorHubDownload([asset('creator-hub-mac-x64.dmg')], device(MAC_UA))?.arch).toBe('amd64')
  })

  it('offers nothing where no installer ships', () => {
    expect(pickCreatorHubDownload(assets, device('Mozilla/5.0 (X11; Linux x86_64)'))).toBeNull()
    expect(pickCreatorHubDownload([], device(WINDOWS_UA))).toBeNull()
    expect(pickCreatorHubDownload([asset('creator-hub-mac-arm64.dmg')], device(MAC_UA, 'intel'))).toBeNull()
    // iPadOS Safari: a Mac user agent on a touch screen.
    expect(pickCreatorHubDownload(assets, device(MAC_UA, 'apple_silicon', 5))).toBeNull()
  })

  it('ignores an asset served from anywhere but the Creator Hub releases', () => {
    const tampered = { name: 'creator-hub-win-x64.exe', browser_download_url: 'javascript:alert(1)' }
    const elsewhere = { name: 'creator-hub-win-x64.exe', browser_download_url: 'https://evil.example/x.exe' }
    expect(pickCreatorHubDownload([tampered, elsewhere], device(WINDOWS_UA))).toBeNull()
  })
})

describe('otherCreatorHubDownloads', () => {
  it("offers every other build, Windows first, so a Mac's chip is never a guess", () => {
    const appleSilicon = pickCreatorHubDownload(assets, device(MAC_UA, 'apple_silicon'))!
    expect(otherCreatorHubDownloads(assets, appleSilicon).map(({ os, arch }) => `${os}/${arch}`)).toEqual([
      'Windows/amd64',
      'macOS/amd64'
    ])
    const windows = pickCreatorHubDownload(assets, device(WINDOWS_UA))!
    expect(otherCreatorHubDownloads(assets, windows).map(({ os, arch }) => `${os}/${arch}`)).toEqual([
      'macOS/arm64',
      'macOS/amd64'
    ])
    expect(otherCreatorHubDownloads([asset('creator-hub-win-x64.exe')], windows)).toEqual([])
  })
})

describe('macArchFromRenderer', () => {
  it('names the chip from the renderer string each browser reports', () => {
    // Chrome prefixes every Mac renderer with "Apple", Intel ones included.
    expect(
      macArchFromRenderer('ANGLE (Apple, ANGLE Metal Renderer: Intel(R) Iris(TM) Plus Graphics, Unspecified Version)')
    ).toBe('intel')
    expect(macArchFromRenderer('ANGLE (Apple, ANGLE Metal Renderer: AMD Radeon Pro 5500M, Unspecified Version)')).toBe(
      'intel'
    )
    expect(macArchFromRenderer('ANGLE (Apple, ANGLE Metal Renderer: Apple M1 Pro, Unspecified Version)')).toBe(
      'apple_silicon'
    )
    expect(macArchFromRenderer('Apple M2')).toBe('apple_silicon')
  })

  it("leaves Safari's masked renderer and an empty one unknown", () => {
    expect(macArchFromRenderer('Apple GPU')).toBe('unknown')
    expect(macArchFromRenderer('')).toBe('unknown')
  })
})

describe('startCreatorHubDownload', () => {
  const assign = vi.fn()
  beforeEach(() => {
    vi.useFakeTimers()
    vi.stubGlobal('location', { ...window.location, assign })
  })
  afterEach(() => {
    cancelCreatorHubRedirect()
    vi.mocked(sendOverviewTrack).mockClear()
    assign.mockClear()
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it("reports the start with sites' funnel props, then opens sites' success page", () => {
    const href = 'https://github.com/decentraland/creator-hub/releases/download/0.50.0/creator-hub-mac-arm64.dmg'
    startCreatorHubDownload({ os: 'macOS', arch: 'arm64', href })

    expect(sendOverviewTrack).toHaveBeenCalledWith(
      'download_started',
      expect.objectContaining({
        download_target: 'creator_hub',
        download_option: 'primary',
        place: 'creators-hero',
        href,
        os: 'macOS',
        arch: 'arm64',
        auth_state: 'anonymous',
        revisit: 0,
        anon_user_id: 'anon-1'
      })
    )
    expect(assign).not.toHaveBeenCalled()
    vi.advanceTimersByTime(SUCCESS_REDIRECT_DELAY_MS)
    expect(assign).toHaveBeenCalledWith(
      'https://decentraland.zone/download/creator-hub-success?os=macOS&arch=arm64&anon_user_id=anon-1'
    )
  })

  it('restarts the redirect for another build, so the success page names the last one chosen', () => {
    const apple = { os: 'macOS', arch: 'arm64', href: `${URL_BASE}/creator-hub-mac-arm64.dmg` } as const
    const intel = { os: 'macOS', arch: 'amd64', href: `${URL_BASE}/creator-hub-mac-x64.dmg` } as const
    expect(startCreatorHubDownload(apple)).toBe(true)
    expect(startCreatorHubDownload(intel, 'alternative')).toBe(true)
    expect(sendOverviewTrack).toHaveBeenLastCalledWith(
      'download_started',
      expect.objectContaining({ arch: 'amd64', download_option: 'alternative' })
    )
    vi.advanceTimersByTime(SUCCESS_REDIRECT_DELAY_MS)
    expect(assign).toHaveBeenCalledTimes(1)
    expect(assign).toHaveBeenCalledWith(expect.stringContaining('arch=amd64'))
  })

  it('starts one download per pending redirect, so a double click counts once', () => {
    const download = { os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' } as const
    expect(startCreatorHubDownload(download)).toBe(true)
    expect(startCreatorHubDownload(download)).toBe(false)
    vi.advanceTimersByTime(SUCCESS_REDIRECT_DELAY_MS)
    expect(sendOverviewTrack).toHaveBeenCalledTimes(1)
    expect(assign).toHaveBeenCalledTimes(1)
  })

  it('stays on the page when the visitor leaves before the redirect', () => {
    startCreatorHubDownload({ os: 'Windows', arch: 'amd64', href: 'https://example.com/creator-hub-win-x64.exe' })
    cancelCreatorHubRedirect()
    vi.advanceTimersByTime(SUCCESS_REDIRECT_DELAY_MS)
    expect(assign).not.toHaveBeenCalled()
  })
})
