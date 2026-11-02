// The web transport against a stub app page under the real web driver and the installed Chrome:
// a command's reply, a failure, and a real click reaching the page. Without Chrome the cases skip,
// unless DCL_E2E_REQUIRE_CHROME=1.
import assert from 'node:assert/strict'
import path from 'node:path'
import { after, before, describe, test } from 'node:test'
import { ConsoleError } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { PACKAGE_ROOT } from '../driver/paths.ts'
import { serveWebApp, type StaticServer } from '../driver/serve.ts'
import { WebConsole, findChrome } from '../driver/web.ts'

const chrome = await findChrome()
const required = process.env.DCL_E2E_REQUIRE_CHROME === '1'
if (!chrome && required) throw new Error('DCL_E2E_REQUIRE_CHROME=1 but no Chrome was found')

describe('WebConsole', { skip: chrome ? false : 'no Chrome installed' }, () => {
  let server: StaticServer
  let web: WebConsole

  before(async () => {
    server = await serveWebApp(path.join(PACKAGE_ROOT, 'test', 'fixtures', 'stub-app'), 0)
    web = await WebConsole.launch({ url: server.url, chrome, headless: true, readyTimeoutMs: 60_000, commandTimeoutMs: 20_000 })
  })

  after(async () => {
    await web?.close()
    await server?.close()
  })

  test('runs a command in the app frame and returns its reply', async () => {
    assert.equal(await web.run('player_position'), '(1, 2, 3)')
    const explorer = new Explorer(web)
    assert.deepEqual(await explorer.position(), { x: 1, y: 2, z: 3 })
    assert.equal((await explorer.worldState()).worldHeld, true)
  })

  test('a failed command is a ConsoleError with the page text', async () => {
    await assert.rejects(web.run('/fail'), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.equal(error.output, 'failed to set location')
      return true
    })
    await assert.rejects(web.run('/nope'), /Command not recognized/)
  })

  test('a HUD click is real input and changes the world state', async () => {
    const explorer = new Explorer(web)
    const result = await explorer.hud.click('Jump In')
    assert.equal(result.input, 'trusted')
    assert.equal((await explorer.worldState()).inWorld, true)
  })
})
