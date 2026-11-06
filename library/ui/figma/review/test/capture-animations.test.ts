import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { settleAnimations } from '../scripts/capture-animations.mjs'

const frame = async () => {}

describe('when capturing a preview with CSS transitions', () => {
  it('should accept an empty frame or animations that already finished', async () => {
    await settleAnimations(() => [], frame)
    await settleAnimations(
      () => [{ playState: 'finished' }, { playState: 'idle' }, { playState: 'paused' }],
      frame
    )
  })
  it('should wait for a running transition before accepting the frame', async () => {
    let finish!: () => void
    let running = true
    let settled = false
    const finished = new Promise<void>((resolve) => {
      finish = () => {
        running = false
        resolve()
      }
    })
    const animation = {
      playState: 'running',
      effect: { getComputedTiming: () => ({ endTime: 250 }) },
      finished
    }
    const waiting = settleAnimations(() => (running ? [animation] : []), frame).then(() => {
      settled = true
    })
    await new Promise((resolve) => setImmediate(resolve))
    assert.equal(settled, false)
    finish()
    await waiting
    assert.equal(settled, true)
  })

  it('should settle transitions started by the preceding transition', async () => {
    let remaining = 2
    const animation = () => ({
      playState: 'running',
      effect: { getComputedTiming: () => ({ endTime: 250 }) },
      get finished() {
        remaining--
        return Promise.resolve()
      }
    })
    await settleAnimations(() => (remaining ? [animation()] : []), frame)
    assert.equal(remaining, 0)
  })

  it('should recheck the frame after a transition is canceled', async () => {
    let running = true
    const animation = {
      playState: 'running',
      effect: { getComputedTiming: () => ({ endTime: 250 }) },
      get finished() {
        running = false
        return Promise.reject(new Error('Transition canceled'))
      }
    }
    await settleAnimations(() => (running ? [animation] : []), frame)
    assert.equal(running, false)
  })

  it('should freeze a looping animation at its initial frame', async () => {
    const animation = {
      playState: 'running',
      currentTime: 123,
      effect: { getComputedTiming: () => ({ endTime: Infinity }) },
      pause() {
        this.playState = 'paused'
      }
    }
    await settleAnimations(() => [animation], frame)
    assert.equal(animation.playState, 'paused')
    assert.equal(animation.currentTime, 0)
  })

  for (const [label, effect] of [
    ['missing effect', undefined],
    ['non-finite duration', { getComputedTiming: () => ({ endTime: NaN }) }]
  ] as const) {
    it(`should reject an animation with ${label}`, async () => {
      await assert.rejects(
        settleAnimations(() => [{ playState: 'running', effect }], frame),
        /Cannot settle preview animation/
      )
    })
  }

  it('should fail when transitions continually restart', async () => {
    await assert.rejects(
      settleAnimations(
        () => [
          {
            playState: 'running',
            effect: { getComputedTiming: () => ({ endTime: 250 }) },
            finished: Promise.resolve()
          }
        ],
        frame
      ),
      /Preview animations did not settle/
    )
  })
})
