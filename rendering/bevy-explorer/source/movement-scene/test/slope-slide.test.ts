import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Animator, Params, Pose } from '../src/animator'
import { CHARACTER_ANIMATOR } from '../src/character-animator'
import { animationGrounded, nextSlideBlend, SLIDE_ANGLE, slopeContact } from '../src/index'

const ANIM = 'assets/animations/'
const DT = 1 / 60

// One tick of what the scene knows about the ground: the angle of a steep face the capsule rests on (0: none), whether
// the model stands on the ground, the distance moved over the tick (m) and the vertical speed (m/s).
type Tick = { steepAngle: number; grounded: boolean; moved: number; vy: number }

// The base layer fed the slope as the scene feeds it each tick, at a jog's movement blend.
class SlopeAnimation {
  readonly animator = new Animator(CHARACTER_ANIMATOR)
  slideBlend = 0
  isGrounded = true

  update(t: Tick, extra: Params = {}): Pose {
    const slope = slopeContact(t.steepAngle, t.grounded, false, t.moved)
    this.slideBlend = nextSlideBlend(this.slideBlend, slope.sliding, DT)
    this.isGrounded = animationGrounded(t.grounded, slope.onSteepSlope, slope.stuck)
    return this.animator.update({
      MovementBlend: 2,
      IsGrounded: this.isGrounded,
      IsFalling: !this.isGrounded && t.vy < -3,
      IsLongFall: !this.isGrounded && t.vy < -8,
      SlideBlend: this.slideBlend,
      ...extra,
    }, DT)
  }
}

// Jogging on the ground, then a slide down a face of `angle` degrees that speeds up at 10 m/s² for `seconds`. Returns
// the clip of each state the slide enters, in order.
function slideDown(angle: number, seconds: number): [string, string][] {
  const a = new SlopeAnimation()
  for (let i = 0; i < 30; i++) a.update({ steepAngle: 0, grounded: true, moved: 0.13, vy: 0 })
  const entered: [string, string][] = []
  for (let i = 1; i <= seconds * 60; i++) {
    const pose = a.update({ steepAngle: angle, grounded: false, moved: 0.1, vy: -10 * i * DT })
    if (pose.entered) entered.push([pose.state, pose.clip ?? pose.scene!])
  }
  return entered
}

test('the slide blend moves 7 per second toward 1 while the avatar slides and back toward 0 after', () => {
  let blend = 0
  for (let i = 1; i <= 6; i++) {
    blend = nextSlideBlend(blend, true, DT)
    assert.ok(Math.abs(blend - 7 * i * DT) < 1e-9, `frame ${i}: ${blend}`)
  }
  for (let i = 0; i < 3; i++) blend = nextSlideBlend(blend, true, DT)
  assert.equal(blend, 1, 'full after 1/7 s')
  assert.ok(Math.abs(nextSlideBlend(1, false, 0.1) - 0.3) < 1e-9)
  assert.equal(nextSlideBlend(0.1, false, 0.013), 0, 'what is left at 0.01 or less is 0')
  assert.ok(nextSlideBlend(0.1, false, 0.0128) > 0.01)
})

test('the avatar slides on a face at least 80 degrees steep it rests on with no ground to stand on', () => {
  assert.deepEqual(slopeContact(85, false, false, 0.1), { onSteepSlope: true, sliding: true, stuck: false })
  assert.deepEqual(slopeContact(SLIDE_ANGLE, false, false, 0.1), { onSteepSlope: true, sliding: true, stuck: false })
  assert.deepEqual(slopeContact(60, false, false, 0.1), { onSteepSlope: true, sliding: false, stuck: false })
  // the model stands on the ground under the centre while the capsule grazes a steep corner
  assert.equal(slopeContact(85, true, false, 0.1).onSteepSlope, false)
  // a jump that rises past a steep face
  assert.equal(slopeContact(85, false, true, 0.1).onSteepSlope, false)
  assert.equal(slopeContact(0, false, false, 0.1).onSteepSlope, false)
  assert.equal(slopeContact(85, false, false, 0.002).stuck, true, 'resting on it without moving')
})

test('IsGrounded is false on a steep slope unless the avatar is stuck there', () => {
  assert.equal(animationGrounded(true, false, false), true)
  assert.equal(animationGrounded(true, true, false), false)
  assert.equal(animationGrounded(false, true, false), false)
  assert.equal(animationGrounded(false, true, true), true)
  // a stuck avatar lands out of the fall states though the model finds no ground
  const a = new SlopeAnimation()
  for (let i = 1; i <= 30; i++) a.update({ steepAngle: 85, grounded: false, moved: 0.1, vy: -10 * i * DT })
  assert.equal(a.isGrounded, false)
  assert.equal(a.animator.state, 'Down')
  assert.equal(a.update({ steepAngle: 85, grounded: false, moved: 0, vy: -6 }).state, 'Grounded')
})

test('a slide down a face too steep to stand on falls into the slide clip, which holds the long fall back', () => {
  assert.deepEqual(slideDown(85, 1.5), [['Down', ANIM + 'Slide.glb']])
})

test('a fall down a steep face below the slide angle plays the fall loop and the long fall', () => {
  assert.deepEqual(slideDown(60, 1.5), [['Down', ANIM + 'Jog_Jump_Fall_Loop.glb'], ['Long Fall', ANIM + 'Long_Fall_Loop.glb']])
})

test('Down goes on to Long Fall only while the slide blend is below 0.5', () => {
  const inDown = () => {
    const a = new Animator(CHARACTER_ANIMATOR)
    a.update({ MovementBlend: 2, IsGrounded: false, IsFalling: true, SlideBlend: 0 }, DT)
    for (let i = 0; i < 60; i++) a.update({ MovementBlend: 2, IsGrounded: false, IsFalling: true, SlideBlend: 0 }, DT)
    assert.equal(a.state, 'Down')
    return a
  }
  const longFall = (slideBlend: number) =>
    ({ MovementBlend: 2, IsGrounded: false, IsFalling: true, IsLongFall: true, SlideBlend: slideBlend })
  const held = inDown()
  for (let i = 0; i < 60; i++) assert.equal(held.update(longFall(0.5), DT).state, 'Down')
  assert.equal(inDown().update(longFall(0.49), DT).state, 'Long Fall')
})
