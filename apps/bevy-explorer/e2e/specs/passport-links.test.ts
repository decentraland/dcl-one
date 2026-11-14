// The QA Automation Suite's PROFILE cases beyond explorer-automation's PassportTests
// (passport.test.ts: open, overview, rename, bio, equipped items): the passport's links, and the
// name colour. The edit form (`Edit profile`) lists a link as two fields, `Link <n> title` and
// `Link <n> URL`, with `Remove link <n>`, and `+ ADD LINK` while fewer than five are set
// (ProfileEditForm.tsx); SAVE deploys the profile. The saved links show on the passport under a
// `Links` heading, each a `link` named "🔗 <title>" (ProfilePassport.tsx).
//
// | Suite case       | Here                                                                            | Class   |
// | Edit about me    | a link added and saved shows on the passport as a link, and is removed again     | Partial |
// | Edit name color  | the colour palette next to the name                                             | Gap     |
//
// Partial, Edit about me: the bio half of the case is passport.test.ts's; this is its Links half. The
// title is run-unique (the editor takes 15 characters), the link is removed and saved again whatever
// the outcome, and an account with five links already is skipped, as none can be added.
// Gap, Edit name color: the Bevy passport has no name colour picker (Notion U5).
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const PASSPORT: Selector = { name: 'Passport sections', role: 'tablist' }
const BIO_FIELD: Selector = { name: 'About me', role: 'textbox' }
const SAVE: Selector = { name: 'SAVE', role: 'button' }
const ADD_LINK: Selector = { name: '+ ADD LINK', role: 'button' }
const LINKS_HEADING: Selector = { name: 'Links', role: 'heading' }
const EQUIPPED_WEARABLES: Selector = { name: 'Equipped Wearables', role: 'heading' }
const LINK_URL = /^Link (\d+) URL$/
const READY_TIMEOUT_MS = 40_000
const SAVE_TIMEOUT_MS = 15_000

const titleField = (n: number): Selector => ({ name: `Link ${n} title`, role: 'textbox' })
const urlField = (n: number): Selector => ({ name: `Link ${n} URL`, role: 'textbox' })

async function openOwnPassport(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Profile', role: 'button' })
  await explorer.hud.wait(PASSPORT, { timeoutMs: READY_TIMEOUT_MS })
  // The links arrive with the rest of the profile, after the identity the passport opens with.
  await explorer.hud.wait({ name: 'Edit name', role: 'button' }, { timeoutMs: READY_TIMEOUT_MS })
  await explorer.hud.wait(EQUIPPED_WEARABLES, { timeoutMs: READY_TIMEOUT_MS })
}

async function closePassport(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Close', role: 'button' })
  await explorer.hud.waitHidden(PASSPORT, 20_000)
}

async function enterEditMode(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Edit profile', role: 'button' })
  await explorer.hud.wait(BIO_FIELD, { state: 'actionable', timeoutMs: READY_TIMEOUT_MS })
}

/** The links of the open edit form, by number, with their URL. */
async function formLinks(explorer: Explorer): Promise<{ n: number; url: string }[]> {
  const links: { n: number; url: string }[] = []
  for (const { name } of await explorer.hud.list('textbox')) {
    const match = LINK_URL.exec(name)
    if (match) links.push({ n: Number(match[1]), url: await explorer.hud.value(urlField(Number(match[1]))) })
  }
  return links
}

/** Press SAVE and wait for the form to close and the profile version to grow. */
async function save(explorer: Explorer): Promise<void> {
  const before = await explorer.profileVersion()
  await explorer.hud.wait(SAVE, { state: 'enabled', timeoutMs: 5_000 })
  // a save is a profile deployment, and the one before it may be seconds old
  await explorer.profileDeployWindow()
  await explorer.hud.click(SAVE)
  await explorer.hud.waitHidden(BIO_FIELD, READY_TIMEOUT_MS)
  await until(() => explorer.profileVersion(), (version) => version > before, { timeoutMs: SAVE_TIMEOUT_MS, what: 'the profile version to grow after the save' })
}

/** Remove the link whose URL is `url` and save; nothing when the profile no longer has it. */
async function removeLink(explorer: Explorer, url: string): Promise<void> {
  await enterEditMode(explorer)
  const link = (await formLinks(explorer)).find((entry) => entry.url === url)
  if (!link) {
    await explorer.hud.click({ name: 'CANCEL', role: 'button' })
    await explorer.hud.waitHidden(BIO_FIELD, 10_000)
    return
  }
  await explorer.hud.click({ name: `Remove link ${link.n}`, role: 'button' })
  await save(explorer)
}

scenario(
  'a link added in the passport editor shows on the passport once saved, and can be removed',
  async ({ explorer, restore, t }) => {
    const stamp = Date.now().toString(36).slice(-6)
    const title = `e2e ${stamp}`
    const url = `https://decentraland.org/?e2e=${stamp}`
    await openOwnPassport(explorer)
    await enterEditMode(explorer)
    if (!(await explorer.hud.isVisible(ADD_LINK))) {
      t.skip('the profile already has five links, the most the editor takes')
      await explorer.hud.click({ name: 'CANCEL', role: 'button' })
      await closePassport(explorer)
      return
    }
    // Set while the scenario's link may be saved: the restore removes it only then.
    let pending = false
    restore(async () => {
      if (!pending) return
      await explorer.escape()
      await openOwnPassport(explorer)
      await removeLink(explorer, url)
      await closePassport(explorer)
    })

    const before = (await formLinks(explorer)).length
    await explorer.hud.click(ADD_LINK)
    const n = before + 1
    await explorer.hud.wait(urlField(n), { state: 'actionable', timeoutMs: 5_000 })
    assert.equal((await explorer.hud.fill(titleField(n), title)).value, title, 'the link title field should hold the typed title')
    assert.equal((await explorer.hud.fill(urlField(n), url)).value, url, 'the link URL field should hold the typed URL')
    pending = true
    await save(explorer)

    await explorer.hud.wait(LINKS_HEADING, { timeoutMs: READY_TIMEOUT_MS })
    await explorer.hud.wait({ name: title, role: 'link', exact: false }, { timeoutMs: READY_TIMEOUT_MS })

    await removeLink(explorer, url)
    pending = false
    assert.equal(await explorer.hud.isVisible({ name: title, role: 'link', exact: false }), false, 'the removed link should no longer show on the passport')
    await closePassport(explorer)
  },
  { timeoutMs: 300_000 }
)

scenario('the colour palette next to a claimed name changes its colour', async () => {}, {
  todo: 'the Bevy passport has no name colour picker (Notion U5)'
})
