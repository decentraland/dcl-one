// Step clock of the horizontal and vertical models. Both models step once per engine frame, by the frame's duration.
import { FRAME_TIME_SMOOTHING, MAX_STEPS, MAX_TICK_TIME } from './constants';

// Time the models advance this tick: from the end of the previous tick's frame to the end of this tick's frame.
export var tickAdvance = 0;
// Duration of this tick's frame (AvatarMovementInfo.stepTime).
export var frameTime = 0;
// Frames the reply is expected to wait before the engine applies it (0..2).
export var replyDelay = 0;
// The same delay, smoothed over ticks (0..2, fractional): consecutive per-tick delays are not correlated.
export var replyDelaySmooth = 0;

var lastFrameTime = 0;
var meanFrameTime = 0;
var meanRatio = 1;

// tickTime: engine time since the previous tick, which ends at the frame before this one.
// lateFrames: frames the previous reply was still in flight after the frame before this one.
export function beginTick(tickTime: number, stepTime: number, previousStepTime: number, lateFrames: number) {
  frameTime = Math.min(Math.max(0, stepTime), MAX_TICK_TIME);
  tickAdvance = Math.max(0, tickTime + frameTime - lastFrameTime);
  lastFrameTime = frameTime;

  // previousStepTime spans the previous dispatch frame to the frame its reply landed on, so its ratio to the frame time
  // is 1 + that reply's delay. An engine that ticks on the frame a late reply lands on dispatches every frame, so there
  // the delay shows as lateFrames. Frame times jitter, so the frame time is smoothed.
  replyDelay = 0;
  if (frameTime <= 0 || previousStepTime <= 0) return;
  meanFrameTime = meanFrameTime > 0 ? meanFrameTime + (frameTime - meanFrameTime) * FRAME_TIME_SMOOTHING : frameTime;
  const ratio = Math.min(previousStepTime, MAX_TICK_TIME) / meanFrameTime + lateFrames;
  meanRatio += (ratio - meanRatio) * FRAME_TIME_SMOOTHING;
  replyDelay = Math.max(0, Math.min(2, Math.round(ratio) - 1));
  replyDelaySmooth = Math.max(0, Math.min(2, meanRatio - 1));
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
