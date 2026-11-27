import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator, Params } from '../src/animator'
import { CHARACTER_ANIMATOR } from '../src/character-animator'
import { EMOTE_MOVE_BLEND, emoteWaits } from '../src/index'

const DT = 1 / 60

test('an avatar standing on the ground may start an emote', () => {
  assert.equal(emoteWaits(new Animator(CHARACTER_ANIMATOR), false, 0), false)
})

test('an emote waits while the movement blend is above its threshold', () => {
  const animator = new Animator(CHARACTER_ANIMATOR)
  assert.equal(emoteWaits(animator, false, EMOTE_MOVE_BLEND), false)
  assert.equal(emoteWaits(animator, false, EMOTE_MOVE_BLEND + 0.01), true)
})

test('an emote waits while the avatar is in the air', () => {
  assert.equal(emoteWaits(new Animator(CHARACTER_ANIMATOR), true, 0), true)
})

test('an emote waits from a jump through its landing until the cross-fade into locomotion is over', () => {
  const animator = new Animator(CHARACTER_ANIMATOR)
  const update = (p: Params) => animator.update({ MovementBlend: 0, ...p }, DT)
  // the update that starts the jump: the cross-fade still shows locomotion, the state it goes to is a jump state
  assert.equal(update({ Jump: true, JumpCount: 1, IsGrounded: false }).state, 'Jump')
  assert.equal(emoteWaits(animator, false, 0), true)
  assert.equal(update({ IsGrounded: true }).state, 'Grounded')
  // then the landing, its cross-fade into locomotion, and a few updates of locomotion
  let entered = -1
  let fade = 0
  for (let n = 0; entered < 0 || n < entered + 30; n++) {
    assert.ok(n < 300, 'the landing ends')
    const pose = update({ IsGrounded: true })
    if (pose.entered && pose.state === 'Movement') [entered, fade] = [n, pose.blend]
    const fading = entered < 0 || (n - entered) * DT < fade
    assert.equal(emoteWaits(animator, false, 0), fading, `update ${n} of the landing, in ${pose.state}`)
  }
})
