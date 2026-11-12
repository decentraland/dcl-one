import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { pathToFileURL } from 'node:url'
import { ConsoleError, FakeConsole, commandOf, normalizeLine, redactLine, scrubCredentials, secretsOf } from '../driver/console.ts'
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
    const error = new ConsoleError('/login_identity eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= --name tester', 'the identity eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= is not base64 JSON')
    assert.equal(error.line, '/login_identity <redacted> --name tester')
    assert.equal(error.output, 'the identity <redacted> is not base64 JSON')
    assert.equal(error.message, '`/login_identity <redacted> --name tester` failed: the identity <redacted> is not base64 JSON')
  })

  test('the credentials are what decodes as an identity, wherever it is, as the web driver has it', async () => {
    const id = 'eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0='
    // `{ "`, pretty-printed JSON and JSON after whitespace, which the commands decode as well
    const spaced = 'eyAiZXBoZW1lcmFsSWRlbnRpdHkiOjF9'
    const pretty = 'ewogICJlcGhlbWVyYWxJZGVudGl0eSI6IDF9'
    const afterWhitespace = ['IHsiZXBoZW1lcmFsSWRlbnRpdHkiOjF9', 'CnsiZXBoZW1lcmFsSWRlbnRpdHkiOjF9', 'CXsiZXBoZW1lcmFsSWRlbnRpdHkiOjF9', 'DQp7ImVwaGVtZXJhbElkZW50aXR5IjoxfQ==']
    const raw = '{"ephemeralIdentity":{"privateKey":"0x56"}}'
    const cases: [string, string[]][] = [
      [`/hud_login ${id} --guest`, [id]],
      [`/hud_login --guest ${id} --look 1`, [id]],
      [`/hud_login --name tester --look 1 ${id} --guest`, [id]],
      [`/login_identity --look 3 ${id}`, [id]],
      [`login_identity ${id}`, [id]],
      [`/hud_login "${id}" --guest`, [id]],
      [`/hud_login '${id}'`, [id]],
      [`/hud_login ${spaced}`, [spaced]],
      [`/login_identity ${pretty} --guest`, [pretty]],
      ...afterWhitespace.map((encoded): [string, string[]] => [`/hud_login ${encoded}`, [encoded]]),
      // a realm's long name is no identity, though base64 characters spell it
      [`/hud_login ${id} --realm example-world-name.dcl.eth`, [id]],
      [`/hud_login ${id} --realm ezra-test-world-2025.dcl.eth --default-on-error`, [id]],
      // in a flag's value slot: the identity, never the word the flags leave over
      [`/hud_login --name ${id} --look 1`, [id]],
      // pasted twice: both
      [`/hud_login ${id} ${pretty}`, [id, pretty]],
      // raw JSON, which the commands refuse after it was logged
      [`/hud_login ${raw} --guest`, [raw]],
      // an empty or short word is no identity: nothing would be safe to replace
      ['/hud_login ""', []],
      ["/hud_login ''", []],
      ['/hud_login "', []],
      ['/hud_login --look 1', []],
      ['/hud_login eyJhIjoxfQ==', []],
      ['/hud_login {}', []],
      // the shape inside a longer base64 run is not where an identity starts
      [`/hud_login x${id}`, []],
      [`/teleport ${id}`, []],
      ['/teleport 1 2', []],
      ['/hud_login', []]
    ]
    const lines = cases.map(([line]) => line)
    const expected = cases.map(([, secrets]) => secrets)
    assert.deepEqual(lines.map(secretsOf), expected)
    assert.equal(redactLine(`/hud_login --guest ${id}`), '/hud_login --guest <redacted>')
    assert.equal(redactLine(`/hud_login --name tester --look 1 ${id}`), '/hud_login --name tester --look 1 <redacted>')
    assert.equal(redactLine(`/hud_login --name ${id} --look 1`), '/hud_login --name <redacted> --look 1')
    assert.equal(redactLine(`/hud_login ${id} ${pretty}`), '/hud_login <redacted> <redacted>')
    assert.equal(redactLine(`/hud_login ${raw} --guest`), '/hud_login <redacted> --guest')
    // a line with no identity is left as it was, not split between every character
    assert.equal(redactLine('/hud_login ""'), '/hud_login ""')
    // the web driver's command line redacts by the same rule
    const driver = (await import(pathToFileURL(WEB_DRIVER).href)) as { secretsOf: (line: string) => string[] }
    assert.deepEqual(lines.map(driver.secretsOf), expected)
  })

  test('keeps a /hud_login payload out as well', () => {
    const error = new ConsoleError('/hud_login eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0= --guest --realm https://realm.example', 'profile fetch failed for eyJlcGhlbWVyYWxJZGVudGl0eSI6MX0=')
    assert.equal(error.line, '/hud_login <redacted> --guest --realm https://realm.example')
    assert.equal(error.output, 'profile fetch failed for <redacted>')
  })

  test('leaves every other line as it is', () => {
    const error = new ConsoleError('/teleport 1 2', 'failed to set location')
    assert.equal(error.line, '/teleport 1 2')
    assert.equal(error.message, '`/teleport 1 2` failed: failed to set location')
  })
})

describe('scrubCredentials', () => {
  test('replaces a room access token in an adapter url and keeps the rest of the line', () => {
    assert.equal(
      scrubCredentials('signed login ok, connecting to inner livekit:wss://rooms.example.test/r?access_token=eyJhbGciOi.x-y_z&mode=1 done'),
      'signed login ok, connecting to inner livekit:wss://rooms.example.test/r?access_token=REDACTED&mode=1 done'
    )
  })

  test('replaces named string fields however they are quoted', () => {
    assert.equal(scrubCredentials('{"privateKey":"0xabc","name":"x"}'), '{"privateKey":"REDACTED","name":"x"}')
    assert.equal(scrubCredentials('identity "{\\"privateKey\\":\\"0xabc\\"}"'), 'identity "{\\"privateKey\\":\\"REDACTED\\"}"')
    assert.equal(scrubCredentials('AuthLink { signature: "0xdead", kind: x }'), 'AuthLink { signature: "REDACTED", kind: x }')
    assert.equal(scrubCredentials('event signature="0xbeef" ok'), 'event signature="REDACTED" ok')
    assert.equal(scrubCredentials('{"accessToken":"t0k"}'), '{"accessToken":"REDACTED"}')
  })

  test('replaces base64 of an identity, as /login_identity takes it', () => {
    const identity = Buffer.from(JSON.stringify({ ephemeralIdentity: { address: '0x1', privateKey: '0x2' }, authChain: [] })).toString('base64')
    assert.equal(scrubCredentials(`> /login_identity ${identity} --guest`), '> /login_identity REDACTED --guest')
  })

  test('replaces the other formats a line can take: bare after =, an Option, a Bearer token, a glued identity', () => {
    assert.equal(scrubCredentials('key privateKey=0xabc next'), 'key privateKey=REDACTED next')
    assert.equal(scrubCredentials('Identity { private_key: Some("0xabc") }'), 'Identity { private_key: Some("REDACTED") }')
    assert.equal(scrubCredentials('authorization: Bearer eyJhbGciOi.payload.sig rest'), 'authorization: Bearer REDACTED rest')
    const identity = Buffer.from(JSON.stringify({ ephemeralIdentity: { privateKey: '0x2' }, authChain: [] })).toString('base64')
    for (const glue of ['a', 'ab', 'abc', 'ab/']) {
      assert.equal(scrubCredentials(`GET ?id=${glue}${identity}`), 'GET ?id=REDACTED', `glued to "${glue}"`)
    }
  })

  test('leaves a line without credentials as it was', () => {
    const line = '08:09:03 iframe log: INFO crates/scene_runner found 4 entities; signature check skipped'
    assert.equal(scrubCredentials(line), line)
    assert.equal(scrubCredentials('a token=abc and ?access_token=&next=1'), 'a token=abc and ?access_token=&next=1')
  })
})
