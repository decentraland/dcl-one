// QA's release checklist, Auth → NEW USERS: signing out, and staying signed in across a restart.
//
// | Checklist case          | Here                                                                       | Class   |
// | Sign Out                | the startup Lobby's profile chip signs out: the engine and the HUD both    | Partial |
// | Login (auto login)      | reopening the client keeps the sign-in                                     | Gap     |
// | Guest login             | a guest persists after reopening the client                                | Gap     |
//
// Partial, Sign Out: the checklist signs out from the profile icon in-world; the Bevy HUD offers Sign
// out only in the startup Lobby's profile chip (the menu's chip has View profile and Exit, and the
// in-world Lobby's has no Sign out). The scenario signs a fresh guest account in with no
// destination, so the HUD stays in that Lobby, signs out there, and checks the engine is signed out
// (`/world_state`) and the HUD left the Lobby for its sign-in page.
//
// Gap, auto login and guest persistence: both need the page reloaded (or the client restarted) with
// the same browser profile, so its stored sign-in is read back; the web driver opens the page once,
// in a fresh profile, and has no reload.
//
// The scenario runs on a client of its own (the engine's own guest, whose HUD stays on its start
// page), never the default session's, which every other scenario shares; it closes it afterwards.
import assert from 'node:assert/strict'
import { buildAuthIdentity, newPrivateKey, spec, until, type Explorer, type Selector } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CONTROL_TIMEOUT_MS = 20_000
const LOBBY_TIMEOUT_MS = 30_000
const NAME = 'e2esignout'

const WELCOME: Selector = { name: 'Welcome', role: 'heading', exact: false }
/** The profile chip's menu row (LobbyProfile.tsx), and the confirmation it asks for first. */
const SIGN_OUT: Selector = { name: 'Sign Out', role: 'menuitem' }
const CONFIRM: Selector = { name: 'Sign out?', role: 'dialog' }
const CONFIRM_SIGN_OUT: Selector = { name: 'Sign Out', role: 'button' }

/** The startup Lobby's profile chip: the one button named after a profile, `<initial> <name>#<tag>`.
 *  A guest's name is the HUD's own pick, not the one the sign-in asked for. */
async function profileChip(explorer: Explorer): Promise<Selector> {
  const chip = (await explorer.hud.list('button')).find((button) => /#[0-9a-f]{4}$/i.test(button.name))
  assert.ok(chip, 'the startup Lobby should show the profile chip')
  return { name: chip.name, role: 'button' }
}

async function press(explorer: Explorer, selector: Selector): Promise<void> {
  await explorer.hud.wait(selector, { state: 'actionable', timeoutMs: CONTROL_TIMEOUT_MS })
  await explorer.hud.click(selector)
}

scenario('the startup Lobby\'s profile chip signs out of the engine and the HUD', async ({ explorer }) => {
  const reply = await explorer.hudLogin(buildAuthIdentity(newPrivateKey()), { name: NAME, look: 1, guest: true })
  assert.equal(reply.phase, 'lobby', `a sign-in with no destination should stay in the Lobby; the HUD answered in phase ${reply.phase}`)
  await explorer.hud.wait(WELCOME, { timeoutMs: LOBBY_TIMEOUT_MS })
  const signedIn = await explorer.worldState()
  assert.equal(signedIn.address?.toLowerCase(), reply.address.toLowerCase(), 'the engine should carry the account the HUD signed in')

  await press(explorer, await profileChip(explorer))
  await press(explorer, SIGN_OUT)
  await explorer.hud.wait(CONFIRM, { timeoutMs: CONTROL_TIMEOUT_MS })
  await press(explorer, CONFIRM_SIGN_OUT)
  await explorer.hud.waitHidden(WELCOME, LOBBY_TIMEOUT_MS)
  const after = await until(() => explorer.worldState(), (state) => !state.signedIn || state.address?.toLowerCase() !== reply.address.toLowerCase(), {
    timeoutMs: LOBBY_TIMEOUT_MS,
    what: 'the engine to sign the account out'
  })
  assert.notEqual(after.address?.toLowerCase(), reply.address.toLowerCase(), 'the engine should no longer carry the signed-out account')
}, { session: { signIn: 'unsigned-guest' }, noEscape: true, closeSession: true, timeoutMs: 240_000 })

scenario('reopening the client keeps the account signed in', async () => {}, {
  todo: 'the web driver opens the page once, in a fresh browser profile, and cannot reload it: the stored sign-in a restart reads back is never read'
})

scenario('a guest is still signed in after reopening the client', async () => {}, {
  todo: 'the web driver opens the page once, in a fresh browser profile, and cannot reload it: the stored guest a restart reads back is never read'
})
