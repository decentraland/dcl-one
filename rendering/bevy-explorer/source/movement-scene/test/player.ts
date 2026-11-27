// The vertical model (src/vertical.ts) driven tick by tick as the scene drives it: frames of 1/60 s, the default
// locomotion settings, a jump key press due on the step after it, and the ground found or not as the test says.
import { beginTick } from '../src/clock'
import { initParameters } from '../src/parameters'
import { initialVState, vadvance, VState } from '../src/vertical'

export const DT = 1 / 60

export class Player {
  readonly s: VState = initialVState()
  private held = false

  constructor() {
    initParameters(undefined, undefined)
    beginTick(DT, DT)
  }

  // One tick that covers `frames` frames, each a step of the model. The launch flags tell of this tick's launches.
  tick(onGround: boolean, frames = 1) {
    this.s.launched = false
    this.s.airLaunched = false
    vadvance(this.s, this.held, false, onGround, false, frames * DT)
  }

  ticks(n: number, onGround: boolean) {
    for (let i = 0; i < n; i++) this.tick(onGround)
  }

  press() {
    this.s.triggerT = this.s.t
    this.held = true
  }

  release() {
    this.held = false
  }
}
