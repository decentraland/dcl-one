import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator, Pose } from '../src/animator'
import { CHARACTER_ANIMATOR } from '../src/character-animator'
import { gliderOpen } from '../src/index'
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
