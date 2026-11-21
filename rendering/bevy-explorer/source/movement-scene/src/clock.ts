// Step clock shared by the horizontal and vertical models. Both integrate in fixed steps
// (FIXED_STEP); this module turns the engine's tick timing into the time each tick covers
// and the reply delay to predict over.
import { FRAME_TIME_SMOOTHING, MAX_TICK_TIME } from './constants';

// Time to add to the step accumulators this tick: from the end of the previous tick's frame
// to the end of this tick's frame.
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

// tickTime: engine time since the previous tick (previousStepTime, clamped), which ends at
// the frame before this one. stepTime: this frame's duration, clamped the same way. lateFrames: frames the previous reply was still in flight
// after the frame before this one (the engine applied an earlier reply there, see index.ts).
export function beginTick(tickTime: number, stepTime: number, previousStepTime: number, lateFrames: number) {
  frameTime = Math.min(Math.max(0, stepTime), MAX_TICK_TIME);
  tickAdvance = Math.max(0, tickTime + frameTime - lastFrameTime);
  lastFrameTime = frameTime;

  // previousStepTime spans the previous dispatch frame through the frame its reply landed on,
  // so its ratio to the frame time is 1 + that reply's delay. An engine that answers a late reply
  // with a tick on the frame it lands on dispatches every frame: there the delay shows as
  // lateFrames instead. Frame times jitter, so the frame time is smoothed; the smoothed ratio gives
  // replyDelaySmooth.
  replyDelay = 0;
  if (frameTime <= 0 || previousStepTime <= 0) return;
  meanFrameTime = meanFrameTime > 0 ? meanFrameTime + (frameTime - meanFrameTime) * FRAME_TIME_SMOOTHING : frameTime;
  const ratio = Math.min(previousStepTime, MAX_TICK_TIME) / meanFrameTime + lateFrames;
  meanRatio += (ratio - meanRatio) * FRAME_TIME_SMOOTHING;
  replyDelay = Math.max(0, Math.min(2, Math.round(ratio) - 1));
  replyDelaySmooth = Math.max(0, Math.min(2, meanRatio - 1));
}
