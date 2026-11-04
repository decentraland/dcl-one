import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { MAX_BOOTS, closeSessions, openSession } from '../driver/session.ts'
import type { Target } from '../driver/target.ts'

/** A native target whose client cannot be started: every boot fails at once. */
const target: Target = {
  baseDomain: 'example.test',
  realm: 'https://realm.example.test',
  position: '0,0',
  transport: 'native',
  webApp: '',
  headed: false,
  windowSize: [800, 600],
  accounts: [],
  worker: 0,
  ports: { web: 0, realm: 0 },
  record: false,
  native: { bin: 'no-such-client-binary', port: 1, args: [] }
}

describe('openSession', () => {
  test('boots again after a failed boot, then fails at once and says so as it happens', async () => {
    const lines: string[] = []
    const original = console.error
    console.error = (line: unknown) => lines.push(String(line))
    try {
      for (let boot = 1; boot <= MAX_BOOTS; boot++) {
        await assert.rejects(openSession({}, target), /could not be started/)
        assert.equal(lines.filter((line) => line.includes(`boot ${boot} of ${MAX_BOOTS} failed: the client no-such-client-binary could not be started`)).length, 1)
      }
      const started = Date.now()
      await assert.rejects(openSession({}, target), /failed to boot 2 times; not booting it again in this run: the client no-such-client-binary could not be started/)
      assert.ok(Date.now() - started < 1000, 'the refusal should not wait on anything')
      assert.equal(lines.filter((line) => line.includes('failed:')).length, MAX_BOOTS, 'a refused boot logs nothing new')
    } finally {
      console.error = original
      await closeSessions()
    }
  })
})
