// How a reaction flies on screen: from beside the reactions button, up and a little to the right.
//
// It is lifted steadily against drag, kicked sideways as it starts and pushed side to side by a
// force that swings once a second. Solved for where that leaves it at the end of its life, the
// flight is a near-steady rise, a drift that eases out and a small sway: the three movements the
// lane animates (ReactionLane.module.css), each on the compositor. Lengths are px at the HUD's
// reference size.

/** From the middle of the reactions button to where a reaction starts, and how far either side. */
const START = { x: 40, y: 0 }
const START_SPREAD = { x: 8, y: 4 }
/** Seconds. */
const LIFETIME = { min: 2.04, max: 2.54 }
/** Width once grown. */
const SIZE = { min: 30, max: 40 }
/** Speed at the start, px a second: sideways and up. */
const KICK = { min: 40, max: 60 }
const UP = { min: 175, max: 200 }
/** Steady acceleration, px a second squared: up and sideways. */
const LIFT = 201
const PUSH = 1
/** What slows it, per second. */
const DRAG = 1.2
/** The swinging force, px a second squared, and its swings a second. */
const SWAY_FORCE = 130
const SWAY_FREQUENCY = 1
/** The most reactions in flight; one that would make more is not shown. */
export const MAX_FLYING = 30

const TURN = 2 * Math.PI
const SWAY_RATE = SWAY_FREQUENCY * TURN
/** How far the swinging force moves a reaction either side, and how far its movement lags the force. */
const SWAY_REACH = SWAY_FORCE / (SWAY_RATE * Math.hypot(DRAG, SWAY_RATE))
const SWAY_LAG = Math.atan2(SWAY_RATE, DRAG)

/** One swing either way, in seconds: the sway animation's length. */
export const SWAY_PERIOD = 1 / SWAY_FREQUENCY
/** Half the width of the sway. */
export const SWAY = SWAY_REACH

export interface ReactionFlight {
  /** Where it starts, from the middle of the reactions button: x to the right, y up. */
  x: number
  y: number
  /** How far it has risen and gone right by the end of its life. */
  rise: number
  drift: number
  /** Seconds. */
  life: number
  /** Its width once grown. */
  size: number
  /** Seconds into its sway it starts. */
  swayAt: number
}

/** Where a reaction started at `kick` px a second sideways and `up` upward, its sway `phase` radians
 *  in, is after `life` seconds: how far up, how far right apart from the sway, and the sway's part. */
export function flightEnd(kick: number, up: number, phase: number, life: number): { rise: number; drift: number; sway: number } {
  const slowed = 1 - Math.exp(-DRAG * life)
  const swaySpeed = SWAY_REACH * SWAY_RATE * Math.sin(phase - SWAY_LAG)
  return {
    rise: (LIFT / DRAG) * life + ((up - LIFT / DRAG) / DRAG) * slowed,
    drift: (PUSH / DRAG) * life + ((kick - swaySpeed - PUSH / DRAG) / DRAG) * slowed,
    sway: SWAY_REACH * (Math.cos(phase - SWAY_LAG) - Math.cos(SWAY_RATE * life + phase - SWAY_LAG))
  }
}

/** A reaction's flight, from `random` (0 to 1, as Math.random). */
export function reactionFlight(random: () => number): ReactionFlight {
  const between = (range: { min: number; max: number }): number => range.min + (range.max - range.min) * random()
  const eitherWay = (reach: number): number => (random() * 2 - 1) * reach
  const life = between(LIFETIME)
  const phase = random() * TURN
  const { rise, drift } = flightEnd(between(KICK), between(UP), phase, life)
  return {
    // the sway swings about its middle: a reaction starts wherever in the swing it is
    x: START.x + eitherWay(START_SPREAD.x) + SWAY_REACH * Math.cos(phase - SWAY_LAG),
    y: START.y + eitherWay(START_SPREAD.y),
    rise,
    drift,
    life,
    size: between(SIZE),
    // the sway animation swings from one side to the other and back, starting at its left
    swayAt: (((phase - SWAY_LAG) / TURN) % 1 + 1) % 1 * SWAY_PERIOD
  }
}
