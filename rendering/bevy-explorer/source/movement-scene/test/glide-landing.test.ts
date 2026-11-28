import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator, Pose } from '../src/animator'
import { CHARACTER_ANIMATOR } from '../src/character-animator'
import { animatorJump, countJumps, gliderOpen } from '../src/index'
import { GlideProp } from '../src/vertical'
import { DT, Player } from './player'

const ANIM = 'assets/animations/'

// The base layer fed the glide as the scene feeds it each tick: the glider's prop and whether the avatar is grounded.
class GlideAnimation {
  readonly animator = new Animator(CHARACTER_ANIMATOR)
  private prev = GlideProp.CLOSED

  update(prop: GlideProp, grounded: boolean): Pose {
    const pose = this.animator.update({
      MovementBlend: 0,
      IsGrounded: grounded,
      StartGliding: prop === GlideProp.OPENING && this.prev !== GlideProp.OPENING,
      IsGliding: gliderOpen(prop),
    }, DT)
    this.prev = prop
    return pose
  }
}

// A jump, an air jump, then the key pressed and held until the glider has glided for a while, the avatar still in the
// air. The animation follows from the glide's press.
function glidingInTheAir() {
  const player = new Player()
  const animation = new GlideAnimation()
  player.ticks(30, true)
  player.press()
  player.tick(true)
  player.ticks(5, false)
  player.release()
  player.ticks(18, false)
  player.press()
  player.tick(false)
  player.release()
  player.ticks(36, false)
  player.press()
  for (let i = 0; i < 90; i++) {
    player.tick(false)
    animation.update(player.s.prop, false)
  }
  assert.equal(player.s.prop, GlideProp.GLIDING, 'the glider is open before the touchdown')
  assert.equal(animation.animator.state, 'Gliding Loop', 'the glide loop plays before the touchdown')
  return { player, animation }
}

test('a glide that touches down with the key held keeps the glider open on that tick and closes it on the next', () => {
  const { player } = glidingInTheAir()
  player.tick(true)
  assert.equal(player.s.prop, GlideProp.GLIDING)
  player.tick(true)
  assert.equal(player.s.prop, GlideProp.CLOSING)
})

test('releasing the key on the touchdown tick closes the glider at once', () => {
  const { player } = glidingInTheAir()
  player.release()
  player.tick(true)
  assert.equal(player.s.prop, GlideProp.CLOSING)
})

test('a touchdown tick that spans several frames keeps the glider open through all its steps', () => {
  const { player } = glidingInTheAir()
  player.tick(true, 3)
  assert.equal(player.s.prop, GlideProp.GLIDING)
  player.tick(true)
  assert.equal(player.s.prop, GlideProp.CLOSING)
})

test('a landing with the glider open plays the landing clip with the glider closing', () => {
  const { player, animation } = glidingInTheAir()
  player.tick(true)
  const pose = animation.update(player.s.prop, true)
  assert.equal(pose.state, 'Grounded')
  assert.equal(pose.clip, ANIM + 'Gliding_Idle_Jump_EndRig.glb')
})

test('a landing on the tick the key is released plays the plain landing clip', () => {
  const { player, animation } = glidingInTheAir()
  player.release()
  player.tick(true)
  const pose = animation.update(player.s.prop, true)
  assert.equal(pose.state, 'Grounded')
  assert.equal(pose.clip, ANIM + 'Idle_Jump_End.glb')
})

test('a jump taken on a touchdown while the glider opens starts no jump state, then or when the glider closes', () => {
  const player = new Player()
  const animator = new Animator(CHARACTER_ANIMATOR)
  let seen = 0
  let prev = GlideProp.CLOSED
  const states: string[] = []
  // the parameters as the scene sets them (index.ts): a launch starts a jump, and the avatar is grounded only on the
  // ground and not launching
  const tick = (onGround: boolean) => {
    player.tick(onGround)
    const s = player.s
    const { jumps, trigger } = countJumps(seen, s.jumpCount, false, undefined, s.launched)
    seen = jumps
    const pose = animator.update({
      MovementBlend: 0,
      Jump: animatorJump(trigger, s.prop),
      JumpCount: jumps,
      IsGrounded: onGround && !s.launched,
      StartGliding: s.prop === GlideProp.OPENING && prev !== GlideProp.OPENING,
      IsGliding: gliderOpen(s.prop),
    }, DT)
    prev = s.prop
    if (pose.entered) states.push(pose.state)
  }
  const ticks = (n: number, onGround: boolean) => { for (let i = 0; i < n; i++) tick(onGround) }
  ticks(30, true)
  player.press()
  tick(true)
  ticks(5, false)
  player.release()
  ticks(18, false)
  player.press()
  tick(false)
  player.release()
  ticks(36, false)
  // the glide press, then the touchdown inside the jump buffer takes it as a jump
  player.press()
  tick(false)
  assert.equal(player.s.prop, GlideProp.OPENING)
  ticks(2, false)
  tick(true)
  assert.ok(player.s.launched, 'the touchdown launches the buffered jump')
  assert.equal(animator.state, 'Gliding_Start')
  const glide = states.length
  for (let i = 0; i < 60 && player.s.prop !== GlideProp.CLOSED; i++) tick(false)
  ticks(30, false)
  assert.equal(states[glide], 'Gliding_End', 'the glider closes with its closing clip')
  assert.ok(!states.slice(glide).includes('Jump'), `no jump state: ${states.slice(glide)}`)
})
