import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { pathToFileURL } from 'node:url'
import { ConsoleError, FakeConsole, commandOf, normalizeLine, redactLine, secretOf } from '../driver/console.ts'
import { WEB_DRIVER } from '../driver/paths.ts'

describe('normalizeLine', () => {
  test('adds the slash and collapses whitespace', () => {
    assert.equal(normalizeLine('teleport  10   20 '), '/teleport 10 20')
    assert.equal(normalizeLine('/world_state'), '/world_state')
  })

  test('refuses an empty line', () => {
    assert.throws(() => normalizeLine('   '), /empty console command/)
  })

  test('commandOf is the name without the slash', () => {
    assert.equal(commandOf('/hud_click Backpack'), 'hud_click')
    assert.equal(commandOf('world_state'), 'world_state')
  })
})

describe('FakeConsole', () => {
  test('answers from the table and records the lines', async () => {
    const fake = new FakeConsole({ world_state: '{"inWorld":true}', echo: (line) => line.slice(6) })
    assert.equal(await fake.run('world_state'), '{"inWorld":true}')
    assert.equal(await fake.run('/echo hello  there'), 'hello there')
    assert.deepEqual(fake.lines, ['/world_state', '/echo hello there'])
    assert.deepEqual(fake.sent('echo'), ['/echo hello there'])
  })

  test('an unknown command fails as the engine does', async () => {
    const fake = new FakeConsole({ time: 'time 12:0 -> 12:0, speed 0 (elapsed: 43200)' })
    await assert.rejects(fake.run('/nope 1'), (error: unknown) => {
      assert.ok(error instanceof ConsoleError)
      assert.equal(error.line, '/nope 1')
      assert.match(error.output, /Command not recognized: `\/nope`. Recognized commands: \["\/time"\]/)
      return true
    })
  })

  test('a reply function may fail the command', async () => {
    const fake = new FakeConsole({
      teleport: (line) => {
        throw new ConsoleError(line, 'failed to set location')
      }
    })
    await assert.rejects(fake.run('/teleport 1 2'), /failed to set location/)
  })
})

describe('ConsoleError', () => {
  test('keeps a login payload out of its line, its output and its message', () => {
    const error = new ConsoleError('/login_identity eyJhIjoxfQ== --name tester', 'the identity eyJhIjoxfQ== is not base64 JSON')
    assert.equal(error.line, '/login_identity <redacted> --name tester')
    assert.equal(error.output, 'the identity <redacted> is not base64 JSON')
    assert.equal(error.message, '`/login_identity <redacted> --name tester` failed: the identity <redacted> is not base64 JSON')
  })

  test('the credential is the first word that is not a bare flag, as the web driver has it', async () => {
    const lines = [
      '/hud_login eyJhIjoxfQ== --guest',
      '/hud_login --guest eyJhIjoxfQ== --look 1',
      '/hud_login --name tester --look 1 eyJhIjoxfQ== --guest',
      '/login_identity --profiles https://x.example/lambdas eyJhIjoxfQ==',
      'login_identity eyJhIjoxfQ==',
      '/teleport 1 2',
      '/hud_login'
    ]
    const expected = ['eyJhIjoxfQ==', 'eyJhIjoxfQ==', 'eyJhIjoxfQ==', 'eyJhIjoxfQ==', 'eyJhIjoxfQ==', undefined, undefined]
    assert.deepEqual(lines.map(secretOf), expected)
    assert.equal(redactLine('/hud_login --guest eyJhIjoxfQ=='), '/hud_login --guest <redacted>')
    assert.equal(redactLine('/hud_login --name tester --look 1 eyJhIjoxfQ=='), '/hud_login --name tester --look 1 <redacted>')
    // the web driver's command line redacts by the same rule
    const driver = (await import(pathToFileURL(WEB_DRIVER).href)) as { secretOf: (line: string) => string | undefined }
    assert.deepEqual(lines.map(driver.secretOf), expected)
  })

  test('keeps a /hud_login payload out as well', () => {
    const error = new ConsoleError('/hud_login eyJhIjoxfQ== --guest --realm https://realm.example', 'profile fetch failed for eyJhIjoxfQ==')
    assert.equal(error.line, '/hud_login <redacted> --guest --realm https://realm.example')
    assert.equal(error.output, 'profile fetch failed for <redacted>')
  })

  test('leaves every other line as it is', () => {
    const error = new ConsoleError('/teleport 1 2', 'failed to set location')
    assert.equal(error.line, '/teleport 1 2')
    assert.equal(error.message, '`/teleport 1 2` failed: failed to set location')
  })
})
