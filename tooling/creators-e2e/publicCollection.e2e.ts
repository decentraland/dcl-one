import { afterEach, describe, expect, it } from 'vitest'
import { linkedCollection, publishedCollection, thirdParty, wearable } from './fixtures'
import { launchApp, type App } from './helpers/app'
import { byTestId, waitForTestId } from './helpers/dom'
import { TEST_ADDRESS } from './helpers/session'

let app: App | undefined
afterEach(async () => {
  await app?.close()
  app = undefined
})

const STRANGER = '0x' + 'be'.repeat(20)
const shared = publishedCollection({ name: 'Stranger Hats', eth_address: STRANGER })
const seed = { collections: [shared], items: [wearable({ collection_id: shared.id, eth_address: STRANGER })] }
const editorPath = `/collections/editor?collection=${shared.id}`

describe('a published collection opened by link without a role on it', () => {
  it('keeps its detail page private', async () => {
    app = await launchApp({ path: `/collections/${shared.id}`, builder: seed })
    await waitForTestId(app.page, 'collection-not-found')
  })

  it('keeps its detail page private from curators too', async () => {
    app = await launchApp({ path: `/collections/${shared.id}`, builder: { ...seed, committee: [TEST_ADDRESS] } })
    await waitForTestId(app.page, 'collection-not-found')
  })

  it('opens in the editor with nowhere to go back to', async () => {
    app = await launchApp({ path: editorPath, builder: seed })
    await waitForTestId(app.page, 'items-sidebar-collection')
    expect(await app.page.$(byTestId('items-sidebar-back'))).toBeNull()
  })

  it('has no back link on a phone either', async () => {
    app = await launchApp({ path: editorPath, builder: seed, viewport: 'mobile' })
    await waitForTestId(app.page, 'mobile-editor-collection')
    expect(await app.page.$(byTestId('mobile-editor-back'))).toBeNull()
  })

  it('takes a curator back to the curation list', async () => {
    app = await launchApp({ path: editorPath, builder: { ...seed, committee: [TEST_ADDRESS] } })
    const back = await waitForTestId(app.page, 'items-sidebar-back')
    expect(await back.evaluate(el => new URL((el as HTMLAnchorElement).href).pathname)).toBe('/curation')
    expect(await back.evaluate(el => el.getAttribute('aria-label'))).toBe('Back to curation')
  })
})

describe('a published collection opened by someone with a role on it', () => {
  it('opens the detail page for a sender', async () => {
    const sent = publishedCollection({ name: 'Sent Hats', eth_address: STRANGER, minters: [TEST_ADDRESS] })
    app = await launchApp({ path: `/collections/${sent.id}`, builder: { collections: [sent] } })
    await waitForTestId(app.page, 'back-to-collections')
  })
})

describe('a published linked collection', () => {
  const linked = linkedCollection({ name: 'Brand X Hats' })
  const linkedSeed = (managers: string[]) => ({
    collections: [linked],
    items: [wearable({ collection_id: linked.id, eth_address: '' })],
    thirdParties: [thirdParty({ managers })]
  })

  it('keeps its detail page from anyone who is not a manager of its third party', async () => {
    app = await launchApp({ path: `/collections/${linked.id}`, builder: linkedSeed([STRANGER]) })
    await waitForTestId(app.page, 'collection-not-found')
  })

  it('takes a manager back from the editor to the detail page', async () => {
    app = await launchApp({
      path: `/collections/editor?collection=${linked.id}`,
      builder: linkedSeed([TEST_ADDRESS])
    })
    const back = await waitForTestId(app.page, 'items-sidebar-back')
    expect(await back.evaluate(el => new URL((el as HTMLAnchorElement).href).pathname)).toBe(
      `/collections/${linked.id}`
    )
  })

  it('opens in the editor with nowhere to go back to for anyone else', async () => {
    app = await launchApp({ path: `/collections/editor?collection=${linked.id}`, builder: linkedSeed([STRANGER]) })
    await waitForTestId(app.page, 'items-sidebar-collection')
    expect(await app.page.$(byTestId('items-sidebar-back'))).toBeNull()
  })
})
