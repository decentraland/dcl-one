// How the passport's avatar turns under a drag and coasts after it (bridge-scene/src/domains/passportTurn).
import { describe, expect, it } from 'vitest'
import { MAX_ANGULAR_VELOCITY, dragTurn, restingTurn, stepTurn } from '../../bridge-scene/src/domains/passportTurn'

const FRAME = 1 / 60

describe('when the passport avatar is dragged', () => {
  it('should move its speed a frame closer to the drag, a pixel a degree, against the pointer', () => {
    const turn = dragTurn(restingTurn(), 6, FRAME)
    // the drag's own speed is -360 degrees a second; a frame of one second's inertia takes 1/60 of the way
    expect(turn.velocity).toBeCloseTo(-360 * FRAME, 6)
    expect(turn.dragging).toBe(true)
  })

  it('should never turn faster than the top speed', () => {
    let turn = restingTurn()
    for (let i = 0; i < 600; i++) turn = dragTurn(turn, -400, FRAME)
    expect(turn.velocity).toBe(MAX_ANGULAR_VELOCITY)
  })

  it('should ignore a drag with no time in it', () => {
    const turn = restingTurn()
    expect(dragTurn(turn, 10, 0)).toBe(turn)
    expect(dragTurn(turn, Number.NaN, FRAME)).toBe(turn)
  })

  it('should turn by its speed each frame while the drag goes on', () => {
    const dragged = { velocity: 120, dragging: true, sinceDrag: 0 }
    const { turn, degrees } = stepTurn(dragged, FRAME)
    expect(degrees).toBeCloseTo(2, 6)
    expect(turn.velocity).toBe(120)
    expect(turn.dragging).toBe(true)
  })
})

describe('when the drag stops', () => {
  it('should keep its speed for a tenth of a second, then count the drag as over', () => {
    let turn = { velocity: 300, dragging: true, sinceDrag: 0 }
    for (let i = 0; i < 6; i++) turn = stepTurn(turn, FRAME).turn
    expect(turn.dragging).toBe(true)
    expect(turn.velocity).toBe(300)
    turn = stepTurn(turn, FRAME).turn
    expect(turn.dragging).toBe(false)
  })

  it('should lose 900 degrees a second each second until it stands still', () => {
    const coasting = { velocity: -450, dragging: false, sinceDrag: 1 }
    const first = stepTurn(coasting, 0.1)
    expect(first.turn.velocity).toBeCloseTo(-360, 6)
    expect(first.degrees).toBeCloseTo(-36, 6)
    let turn = first.turn
    for (let i = 0; i < 10; i++) turn = stepTurn(turn, 0.1).turn
    expect(turn.velocity).toBe(0)
    expect(stepTurn(turn, 0.1).degrees).toBe(0)
  })
})
