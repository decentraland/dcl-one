// How the passport's avatar turns under a drag and coasts after it: the drag sets a speed the avatar picks
// up over a second, a pixel a degree, and loses again once the drag stops. Pure, so react-web's vitest
// covers it.

/** Degrees a second, either way. */
export const MAX_ANGULAR_VELOCITY = 900
/** Degrees a second lost each second once the drag has stopped. */
const DECELERATION = 900
/** Seconds without a drag before the avatar coasts. */
const DRAG_TIMEOUT = 0.1
const INERTIA = 1
const STILL = 0.01

export type Turn = {
  /** Degrees a second. */
  velocity: number
  dragging: boolean
  /** Seconds since the last drag. */
  sinceDrag: number
}

export const restingTurn = (): Turn => ({ velocity: 0, dragging: false, sinceDrag: 0 })

/** A drag of `dx` screen pixels over `seconds`: the velocity moves toward the drag's own, a pixel a degree. */
export function dragTurn(turn: Turn, dx: number, seconds: number): Turn {
  if (!Number.isFinite(dx) || !Number.isFinite(seconds) || seconds <= 0) return turn
  const target = -dx / seconds
  const velocity = turn.velocity + (target - turn.velocity) * Math.min(1, seconds / INERTIA)
  return { velocity: Math.min(MAX_ANGULAR_VELOCITY, Math.max(-MAX_ANGULAR_VELOCITY, velocity)), dragging: true, sinceDrag: 0 }
}

/** A frame of `dt` seconds: the turn after it, and the degrees the avatar turned in it. */
export function stepTurn(turn: Turn, dt: number): { turn: Turn; degrees: number } {
  const sinceDrag = turn.sinceDrag + dt
  const dragging = turn.dragging && sinceDrag <= DRAG_TIMEOUT
  let velocity = turn.velocity
  if (!dragging) {
    const speed = Math.abs(velocity) - INERTIA * DECELERATION * dt
    velocity = speed <= 0 ? 0 : Math.sign(velocity) * speed
  }
  return { turn: { velocity, dragging, sinceDrag }, degrees: Math.abs(velocity) > STILL ? velocity * dt : 0 }
}
