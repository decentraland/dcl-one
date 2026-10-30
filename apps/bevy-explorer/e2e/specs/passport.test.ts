// explorer-automation `PassportTests` (5): the own passport opens from the sidebar, shows its
// header and modules, and its name and bio can be edited and restored.
//
// | Unity test                           | Here                                                                         | Class   |
// | TestOpenOwnPassportFromSidebar       | the rail's Profile button opens the passport, Close closes it                | Port    |
// | TestPassportOverviewShowsCoreContent | the header's controls, the engine's name and address, and the module titles   | Partial |
// | TestEditUserNameAndRevert            | rename twice through Edit Username and back, reading the engine's name       | Partial |
// | TestEditAboutMeAndRestore            | edit the bio, reopen to check it persisted, restore it                       | Partial |
// | TestEquippedItemsShownInPassport     | the Equipped Wearables module is on screen and the avatar wears something    | Partial |
//
// Port, TestOpenOwnPassportFromSidebar: the rail's `Profile` button opens the passport itself;
// there is no profile menu and no "View PROFILE" step.
//
// The passport's header text (name, `#` tag, address snippet), bio text and equipped tiles are
// plain `span`, `p` and `div` in the HUD: they have no role, so `/hud_list` and `/hud_read` cannot
// name them. What is addressable is the controls (`Edit name`, `Copy address`, `Edit profile`, the
// tabs), the `h2` headings (`About Me` only while a bio is set; `Equipped Wearables` only while
// something is equipped) and, in edit mode, the `About me` field's value. Hence the Partials:
// - Overview: the name and the address come from `/get_user_data`, not the header; the `#` tag and
//   the address snippet become the `Copy address` control; "About Me" and "Equipped Items" titles
//   become the `Equipped Wearables` heading and the `Edit profile` card, since the bio's text and
//   its empty state are not addressable.
// - Rename: the discriminator (the address's last four digits, never part of the stored name)
//   becomes "the address did not change". The name is read from `/get_user_data`, which stores it
//   bare. The editor's rule is Unity's 1 to 15 characters and also letters and numbers only
//   (isValidName), so a longer or punctuated original is replaced by `devauto`, as Unity does.
// - Bio: the displayed bio is read through the edit form's `About me` value (entered and left with
//   CANCEL), and an empty bio restores as empty, not as Unity's "No intro." text.
// - Equipped items: the heading renders only with at least one tile, and `/avatar_state` confirms
//   the avatar selects wearables; the tiles themselves cannot be counted.
import assert from 'node:assert/strict'
import { spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const PASSPORT: Selector = { name: 'Passport sections', role: 'tablist' }
const BIO_FIELD: Selector = { name: 'About me', role: 'textbox' }
const USERNAME_FIELD: Selector = { name: 'Username', role: 'textbox' }
const SAVE: Selector = { name: 'SAVE', role: 'button' }
const EQUIPPED_WEARABLES: Selector = { name: 'Equipped Wearables', role: 'heading' }
const ABOUT_ME_HEADING: Selector = { name: 'About Me', role: 'heading' }

const READY_TIMEOUT_MS = 40_000
const MAX_NAME_LENGTH = 15
const NAME_PATTERN = new RegExp(`^[A-Za-z0-9]{1,${MAX_NAME_LENGTH}}$`)
const BASE_NAME = 'devauto'
const TEMP_NAME = 'devauto2'
const BIO = 'Automated passport test bio'

/** The part of a name before its `#tag`: the profile stores a non-claimed name bare. */
const baseOf = (name: string): string => name.split('#')[0] ?? name

async function openOwnPassport(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Profile', role: 'button' })
  await explorer.hud.wait(PASSPORT, { timeoutMs: READY_TIMEOUT_MS })
  // The edit controls arrive with the session's profile.
  await explorer.hud.wait({ name: 'Edit name', role: 'button' }, { timeoutMs: READY_TIMEOUT_MS })
}

async function closePassport(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Close', role: 'button' })
  await explorer.hud.waitHidden(PASSPORT, 20_000)
}

/** Type a name into the Edit Username popup, save it, and wait for the engine to carry it. */
async function renameUser(explorer: Explorer, name: string): Promise<void> {
  const { hud } = explorer
  await hud.click({ name: 'Edit name', role: 'button' })
  await hud.wait(USERNAME_FIELD, { state: 'actionable', timeoutMs: READY_TIMEOUT_MS })
  const typed = await hud.fill(USERNAME_FIELD, name)
  assert.equal(typed.value, name, `the Username field should hold "${name}" after typing`)
  await hud.wait(SAVE, { state: 'enabled', timeoutMs: 5_000 })
  await hud.click(SAVE)
  await hud.waitHidden(USERNAME_FIELD, READY_TIMEOUT_MS)
  await until(() => explorer.userData(), (data) => baseOf(data.name) === name, { timeoutMs: 15_000, what: `the profile name to read "${name}"` })
}

async function enterEditMode(explorer: Explorer): Promise<void> {
  await explorer.hud.click({ name: 'Edit profile', role: 'button' })
  await explorer.hud.wait(BIO_FIELD, { state: 'actionable', timeoutMs: READY_TIMEOUT_MS })
}

/** The bio as the edit form holds it, entered and left (nothing is dirty, so CANCEL asks nothing). */
async function readBio(explorer: Explorer): Promise<string> {
  await enterEditMode(explorer)
  const bio = await explorer.hud.value(BIO_FIELD)
  await explorer.hud.click({ name: 'CANCEL', role: 'button' })
  await explorer.hud.waitHidden(BIO_FIELD, 10_000)
  return bio
}

/** Replace the bio and save: the form closes once the save landed, and the profile version grows. */
async function saveBio(explorer: Explorer, text: string): Promise<void> {
  const { hud } = explorer
  const before = await explorer.profileVersion()
  await enterEditMode(explorer)
  const typed = await hud.fill(BIO_FIELD, text)
  assert.equal(typed.value, text, 'the About me field should hold the typed bio')
  await hud.wait(SAVE, { state: 'enabled', timeoutMs: 5_000 })
  await hud.click(SAVE)
  await hud.waitHidden(BIO_FIELD, READY_TIMEOUT_MS)
  await until(() => explorer.profileVersion(), (version) => version > before, { timeoutMs: 15_000, what: 'the profile version to grow after the bio was saved' })
}

scenario('the rail Profile button opens the own passport and Close closes it', async ({ explorer }) => {
  await openOwnPassport(explorer)
  assert.ok(await explorer.hud.isVisible(PASSPORT), 'the passport should be open after the Profile button')
  await closePassport(explorer)
  assert.equal(await explorer.hud.isVisible(PASSPORT), false, 'the passport should be closed after Close')
})

scenario('the own passport shows its header controls and its About and Equipped modules', async ({ explorer }) => {
  await openOwnPassport(explorer)
  const data = await explorer.userData()
  assert.ok(baseOf(data.name).trim().length > 0, 'the profile name should not be empty')
  assert.match(data.address, /^0x[0-9a-f]{40}$/i, 'the wallet address should be a 0x address')
  for (const control of [
    { name: 'Edit name', role: 'button' },
    { name: 'Copy name', role: 'button' },
    { name: 'Copy address', role: 'button' },
    { name: 'OVERVIEW', role: 'tab' },
    { name: 'Edit profile', role: 'button' }
  ] satisfies Selector[]) {
    assert.ok(await explorer.hud.isVisible(control), `the passport should show its ${control.name} ${control.role}`)
  }
  await explorer.hud.wait(EQUIPPED_WEARABLES, { timeoutMs: READY_TIMEOUT_MS })
  await closePassport(explorer)
})

scenario(
  'renaming through Edit Username updates the profile name, keeps the address and can be reverted',
  async ({ explorer }) => {
    await openOwnPassport(explorer)
    const original = await explorer.userData()
    const originalName = baseOf(original.name)
    const baseName = NAME_PATTERN.test(originalName) ? originalName : BASE_NAME
    if (baseName !== originalName) await renameUser(explorer, baseName)
    assert.equal(baseOf((await explorer.userData()).name), baseName, 'the profile should carry the baseline name')

    const tempName = baseName === BASE_NAME ? TEMP_NAME : BASE_NAME
    await renameUser(explorer, tempName)
    const renamed = await explorer.userData()
    assert.equal(baseOf(renamed.name), tempName, 'the profile should carry the new name after saving the rename')
    assert.equal(renamed.address, original.address, 'the address-derived discriminator must not change on rename')

    await renameUser(explorer, baseName)
    assert.equal(baseOf((await explorer.userData()).name), baseName, 'the profile should carry the baseline name again after the revert')
    await closePassport(explorer)
  },
  { timeoutMs: 240_000 }
)

scenario(
  'the bio can be edited, persists across closing and reopening the passport, and can be restored',
  async ({ explorer }) => {
    await openOwnPassport(explorer)
    const originalBio = await readBio(explorer)
    // A bio left over from an interrupted run would make the edit a no-op, which SAVE refuses.
    const newBio = originalBio.trim() === BIO ? `${BIO} 2` : BIO
    await saveBio(explorer, newBio)
    assert.equal(await readBio(explorer), newBio, 'the bio should read the new text after the edit')
    assert.ok(await explorer.hud.isVisible(ABOUT_ME_HEADING), 'the About Me card should show its title once the bio is set')

    await closePassport(explorer)
    await openOwnPassport(explorer)
    assert.equal(await readBio(explorer), newBio, 'the bio should persist after closing and reopening the passport')

    await saveBio(explorer, originalBio)
    assert.equal((await readBio(explorer)).trim(), originalBio.trim(), 'the bio should read the original text after the restore')
    assert.equal(await explorer.hud.isVisible(ABOUT_ME_HEADING), originalBio.trim() !== '', 'the About Me title should show exactly when a bio is set')
    await closePassport(explorer)
  },
  { timeoutMs: 300_000 }
)

scenario('the Equipped Wearables module is shown for an avatar that wears something', async ({ explorer }) => {
  await openOwnPassport(explorer)
  await explorer.hud.wait(EQUIPPED_WEARABLES, { timeoutMs: READY_TIMEOUT_MS })
  const avatar = await explorer.myAvatar()
  assert.ok(avatar.selected.wearables.length > 0, 'the avatar should select at least one wearable for the module to list')
  await closePassport(explorer)
})
