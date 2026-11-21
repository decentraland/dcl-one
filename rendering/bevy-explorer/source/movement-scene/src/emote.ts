import { InputAction, inputSystem, MovementAnimation } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math';
import { movementAxis } from './horizontal';
import { disableOrientation, maxGroundJumps } from './parameters';

// Movement input (length of the input axis) above which a played emote ends.
const EMOTE_END_INPUT = 0.1;

// The engine plays a triggered emote only over an idle movement animation: a non-idle one ends the emote, and an emote
// requested over a non-idle one is dropped. The scene cannot see whether an emote plays, so it states when one may:
// movement input, or jump input while a ground jump is allowed, makes the published animation non-idle even when it
// moves nothing (a wall, a landing stun, a slope too steep), so the emote ends on the input, not on the motion. With
// every gait disabled the input moves nothing and keeps the emote. Airborne clips are never idle.
export function endEmoteOnInput(animation: MovementAnimation) {
  if (!animation.idle) return;
  const move = !disableOrientation && Vector3.lengthSquared(movementAxis) > EMOTE_END_INPUT * EMOTE_END_INPUT;
  const jump = maxGroundJumps > 0 && inputSystem.isPressed(InputAction.IA_JUMP);
  if (move || jump) animation.idle = false;
}
