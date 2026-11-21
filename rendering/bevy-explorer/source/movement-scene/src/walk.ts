import { ColliderLayer, engine, Entity, InputAction, inputSystem, Raycast, RaycastQueryType, RaycastShape, raycastSystem, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'
import { playerPosition, playerRotation, tick, time } from '.'
import { GROUND_SNAP_HEIGHT, PLAYER_COLLIDER_RADIUS } from './constants'

var walkTarget: Vector3 | null = null;
var walkTolerance = 0.5;

// Drop detection: entity positioned ahead of player (player-local space), casts straight down.
// maxDistance is set to 0 when walk is inactive so the cast does nothing.
var dropCaster: Entity;
var dropHitDistance = Infinity;

const DROP_LOOK_AHEAD = PLAYER_COLLIDER_RADIUS * 2;  // how far ahead to place the cast origin
const DROP_MAX_DIST = GROUND_SNAP_HEIGHT * 3;         // max cast distance when active
const DROP_THRESHOLD = GROUND_SNAP_HEIGHT;            // stop if ground ahead is this far below player feet

// Stuck detection
var stuckCheckTime = 0;
var stuckCheckXZDist = Infinity;
const STUCK_CHECK_INTERVAL = 0.5;  // seconds between progress checks
const STUCK_MIN_PROGRESS = 0.15;   // minimum XZ distance decrease required per interval

var walkStartTick = -1;
var walkResult: boolean | undefined = undefined;

// Engine integration: tracks whether the current walk was initiated by AvatarMovementInfo.walkTarget.
var engineWalkActive = false;
var engineWalkTarget: Vector3 | null = null;  // last target received from engine, for change detection

// Called each frame from initFrame with the current AvatarMovementInfo values.
// Handles engine-initiated walks: starts, continues, and cancels them.
export function updateEngineWalk(target: Vector3 | undefined, threshold: number | undefined) {
  if (target !== undefined) {
    engineWalkActive = true;
    const changed = engineWalkTarget === null ||
      Math.abs(target.x - engineWalkTarget.x) > 0.001 ||
      Math.abs(target.y - engineWalkTarget.y) > 0.001 ||
      Math.abs(target.z - engineWalkTarget.z) > 0.001;
    if (engineWalkTarget === null) {
      engineWalkTarget = { ...target };
    } else {
      Vector3.copyFrom(target, engineWalkTarget);
    }
    if (changed || walkTarget === null) {
      // New target (or walk was idle): start fresh
      walkTarget = { ...target };
      walkTolerance = threshold ?? 0.5;
      stuckCheckTime = time;
      stuckCheckXZDist = Infinity;
      walkStartTick = tick;
      walkResult = undefined;
    }
    // Same target while walk is active: leave all state alone
  } else if (engineWalkActive) {
    // Engine removed walk_target: cancel if still walking
    engineWalkActive = false;
    engineWalkTarget = null;
    if (walkTarget !== null) {
      walkResult = false;
      walkTarget = null;
    }
  }
}

// Returns the walk result (true=success, false=failed, undefined=in-progress or not started).
// Consuming clears it so it is returned for one frame only.
export function consumeWalkResult(): boolean | undefined {
  const result = walkResult;
  walkResult = undefined;
  return result;
}

export function initWalkSystem() {
  dropCaster = engine.addEntity();
  Transform.create(dropCaster, { parent: engine.PlayerEntity, position: { x: 0, y: 0, z: 0 } });

  raycastSystem.registerGlobalDirectionRaycast(
    {
      entity: dropCaster,
      opts: {
        maxDistance: 0,  // disabled until walk is active
        queryType: RaycastQueryType.RQT_HIT_FIRST,
        continuous: true,
        collisionMask: ColliderLayer.CL_PHYSICS,
        shape: RaycastShape.RS_AVATAR,
        includeWorld: true,
        direction: Vector3.Down(),
      }
    },
    (hits) => {
      dropHitDistance = Infinity;
      for (const hit of hits.hits) {
        if (hit.length < dropHitDistance) {
          dropHitDistance = hit.length;
        }
      }
    }
  );
}

var conjugate = Quaternion.Identity();
var localAhead = Vector3.Zero();
var autoWalkAxis = Vector3.Zero();
// Reposition the drop caster ahead of the player (in player-local space) and enable/disable it.
function updateDropCaster(walkAxis: Vector3 | null) {
  const mutDrop = Raycast.getMutable(dropCaster);
  if (walkAxis === null) {
    mutDrop.maxDistance = 0;
    return;
  }

  // Convert world-space look-ahead offset to player-local space via conjugate quaternion.
  conjugate.x = -playerRotation.x;
  conjugate.y = -playerRotation.y;
  conjugate.z = -playerRotation.z;
  conjugate.w = playerRotation.w;
  Vector3.scaleToRef(walkAxis, DROP_LOOK_AHEAD, localAhead);
  Vector3.rotateToRef(localAhead, conjugate, localAhead);
  const position = Transform.getMutable(dropCaster).position;
  position.x = localAhead.x;
  position.y = 0;
  position.z = localAhead.z;

  mutDrop.maxDistance = DROP_MAX_DIST;
}

// Returns a normalized XZ walk axis if auto-walk is active, or null.
// Handles manual-input cancellation and all stopping conditions.
// Call this at the start of updateMovementAxis(); if non-null, use the returned axis directly.
export function getWalkAxis(): Vector3 | null {
  if (walkTarget === null) {
    updateDropCaster(null);
    return null;
  }

  // Any manual directional input cancels the walk and falls through to normal input handling
  if (
    inputSystem.isPressed(InputAction.IA_LEFT) ||
    inputSystem.isPressed(InputAction.IA_RIGHT) ||
    inputSystem.isPressed(InputAction.IA_FORWARD) ||
    inputSystem.isPressed(InputAction.IA_BACKWARD)
  ) {
    walkResult = false;
    walkTarget = null;
    updateDropCaster(null);
    return null;
  }

  const dx = walkTarget.x - playerPosition.x;
  const dz = walkTarget.z - playerPosition.z;
  const xzDist = Math.sqrt(dx * dx + dz * dz);

  // Stop: reached target within tolerance, or exactly on it (a zero tolerance; the axis to it is undefined)
  if (xzDist < walkTolerance || xzDist < 1e-6) {
    walkResult = true;
    walkTarget = null;
    updateDropCaster(null);
    return null;
  }

  // Stop: large drop or void ahead (dropHitDistance is Infinity when nothing is hit).
  // Skip the first tick — the cast just became active and results won't arrive until next frame.
  // Cap the measured drop at playerPosition.y (world floor bounds it when there's no collider).
  // Allow dropping as far as needed to reach the target Y, with GROUND_SNAP_HEIGHT tolerance.
  if (tick > walkStartTick) {
    const cappedDrop = Math.min(dropHitDistance, playerPosition.y);
    const allowedDrop = Math.max(DROP_THRESHOLD, playerPosition.y - walkTarget.y + GROUND_SNAP_HEIGHT);
    if (cappedDrop > allowedDrop) {
      walkResult = false;
      walkTarget = null;
      updateDropCaster(null);
      return null;
    }
  }

  // Stop: not making enough XZ progress toward the target
  if (time - stuckCheckTime > STUCK_CHECK_INTERVAL) {
    const progress = stuckCheckXZDist - xzDist;
    stuckCheckTime = time;
    stuckCheckXZDist = xzDist;
    if (progress < STUCK_MIN_PROGRESS) {
      walkResult = false;
      walkTarget = null;
      updateDropCaster(null);
      return null;
    }
  }

  Vector3.copyFromFloats(dx / xzDist, 0, dz / xzDist, autoWalkAxis);
  updateDropCaster(autoWalkAxis);
  return autoWalkAxis;
}
