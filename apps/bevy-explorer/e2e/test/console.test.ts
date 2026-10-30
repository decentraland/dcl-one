import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { ConsoleError, FakeConsole, commandOf, normalizeLine } from '../driver/console.ts'

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
