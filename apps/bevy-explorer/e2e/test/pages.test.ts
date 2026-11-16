import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { closePage } from '../specs/pages.ts'

describe('closePage', () => {
  test('a restore does not press Escape or deploy when the body already closed the Backpack', async () => {
    const fake = new FakeConsole({ hud_read: '{"found":false}' })
    await closePage(new Explorer(fake), 'backpack')
    assert.equal(fake.sent('hud_read').length, 1)
    assert.deepEqual(fake.sent('press_key'), [])
    assert.deepEqual(fake.sent('get_user_data'), [])
  })

  test('an open page still receives Escape and must become hidden', async () => {
    const fake = new FakeConsole({
      hud_read: '{"found":true,"visible":true}',
      world_state: '{"keyboardClaimed":false}',
      press_key: 'pressed Escape',
      hud_wait: '{"found":false}'
    })
    await closePage(new Explorer(fake), 'events')
    assert.deepEqual(fake.sent('press_key'), ['/press_key Escape'])
    assert.match(fake.sent('hud_wait')[0]!, /"state":"hidden"/)
  })
})
