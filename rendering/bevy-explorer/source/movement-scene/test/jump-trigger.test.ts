import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator } from '../src/animator'
import { CHARACTER_ANIMATOR } from '../src/character-animator'
import { JUMP_COYOTE_TIME } from '../src/constants'
import { countJumps } from '../src/index'
import { DT, Player } from './player'

type JumpStart = { jumps: number; airTime: number }

// Walks off a ledge with no jump key: `air` ticks in the air, then the ground again. Each tick the jump count and the
// base layer are updated as the scene updates them; returns the ticks that start the Jump state.
function walkOff(air: number): JumpStart[] {
  const player = new Player()
  const animator = new Animator(CHARACTER_ANIMATOR)
  let seen = 0
  const starts: JumpStart[] = []
  const tick = (onGround: boolean) => {
    player.tick(onGround)
    const { jumps, trigger } = countJumps(seen, player.s.jumpCount, false, undefined, false)
    seen = jumps
    const pose = animator.update({ MovementBlend: 0, Jump: trigger, JumpCount: jumps, IsGrounded: onGround }, DT)
    if (pose.entered && pose.state === 'Jump') starts.push({ jumps, airTime: player.s.t0 - player.s.lastGroundedEnd })
  }
  for (let i = 0; i < 60; i++) tick(true)
  for (let i = 0; i < air; i++) tick(false)
  for (let i = 0; i < 30; i++) tick(true)
  return starts
}

test('a walk off a ledge that falls past the coyote window starts the jump states once, as one jump', () => {
  const starts = walkOff(30)
  assert.equal(starts.length, 1)
  assert.equal(starts[0].jumps, 1)
  assert.ok(starts[0].airTime >= JUMP_COYOTE_TIME - 1e-6, `started ${starts[0].airTime} s into the fall`)
  assert.ok(starts[0].airTime < JUMP_COYOTE_TIME + DT, `started ${starts[0].airTime} s into the fall`)
})

test('a walk off that lands inside the coyote window starts no jump state', () => {
  assert.deepEqual(walkOff(Math.floor(JUMP_COYOTE_TIME / DT) - 2), [])
})
