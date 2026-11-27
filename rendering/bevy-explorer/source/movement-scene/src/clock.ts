// Step clock of the horizontal and vertical models. Both models step once per engine frame, by the frame's duration.
import { MAX_STEPS, MAX_TICK_TIME } from './constants';

// Time the models advance this tick: from the end of the previous tick's frame to the end of this tick's frame.
export var tickAdvance = 0;
// Duration of this tick's frame (AvatarMovementInfo.stepTime).
export var frameTime = 0;

var lastFrameTime = 0;

// tickTime: engine time since the previous tick, which ends at the frame before this one.
export function beginTick(tickTime: number, stepTime: number) {
  frameTime = Math.min(Math.max(0, stepTime), MAX_TICK_TIME);
  tickAdvance = Math.max(0, tickTime + frameTime - lastFrameTime);
  lastFrameTime = frameTime;
}

// Steps that cover `time`: one per frame of this tick's frame length, with the time split evenly between them.
export function stepsFor(time: number): number {
  if (time <= 1e-9) return 0;
  return frameTime > 0 ? Math.max(1, Math.min(MAX_STEPS, Math.round(time / frameTime))) : 1;
}

// Step length when one step is due and no frame length is known yet.
export function oneStep(): number {
  return frameTime > 0 ? frameTime : DEFAULT_STEP;
}
const DEFAULT_STEP = 1 / 60;
