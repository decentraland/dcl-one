import { afterEach, describe, expect, it } from 'vitest'
import { launchApp, type App } from './helpers/app'
import { byTestId, waitForTestId, waitForText } from './helpers/dom'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const hasHorizontalOverflow = (a: App) =>
  a.page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth)

describe('overview (creator home)', () => {
  it('shows the live scenes and blog rails to a signed-out visitor', async () => {
    app = await launchApp({ path: '/', signedOut: true })
    const { page } = app

    await waitForTestId(page, 'overview-hero')
    await page.$eval(byTestId('overview-live-scenes'), el => el.scrollIntoView())
    await waitForText(page, 'Neon Rooftop')

    const scenes = await page.$$eval(byTestId('overview-scene-card'), cards =>
      cards.map(card => (card as HTMLAnchorElement).href)
    )
    // The genesis-plaza filter and the busiest-first order run on the fixture's two scenes.
    expect(scenes).toEqual([
      'https://decentraland.zone/places/place/10,20',
      'https://decentraland.zone/places/place/-5,33'
    ])

    // Like a visitor, stay on a section until it counts as viewed before scrolling on.
    await app.waitForEvent('Section Viewed', { section_viewed: 'Creators Live Scenes', mobile: false })

    await page.$eval(byTestId('overview-blog'), el => el.scrollIntoView())
    await waitForText(page, 'Creator spotlight')
    await app.waitForEvent('Section Viewed', { section_viewed: 'Creators Blog', mobile: false })
  })

  it('sends the old /overview link to the home page', async () => {
    app = await launchApp({ path: '/overview', signedOut: true })
    await waitForTestId(app.page, 'overview-hero')
    expect(new URL(app.page.url()).pathname).toBe('/')
  })

  it('fits a phone screen', async () => {
    app = await launchApp({ path: '/', signedOut: true, viewport: 'mobile' })
    await waitForTestId(app.page, 'overview-hero')
    expect(await hasHorizontalOverflow(app)).toBe(false)
  })
})
