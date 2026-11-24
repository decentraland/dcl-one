import { ColliderLayer, engine, Entity, RaycastHit, RaycastQueryType, RaycastShape, raycastSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math';
import { playerPosition, prevPlayerPosition, tick } from '.';
import { GROUND_CAST_RANGE, GROUND_SNAP_STEP_DROP, GROUNDED_ANGLE, GROUNDED_HEIGHT, PLAYER_COLLIDER_RADIUS, VEC3_UP, VEC3_ZERO } from './constants';

// updated in raycast update
export var groundNormal = Vector3.Zero();
export var groundDistance = 0;
// ground straight below the centre (point ray): feet height above it (Infinity: none in range) and its normal
export var groundRayDistance = Infinity;
export var groundRayNormal = Vector3.Zero();
// updated in recordGroundPosition @ 100000 - 2
export var grounded = false;
export var prevGrounded = false;

export const GROUNDED_ANGLE_Y_LEN = Math.cos(GROUNDED_ANGLE / 180 * Math.PI)

// a walkable hit within grounded reach of the capsule's bottom
const groundedHit = (hit: RaycastHit) =>
    hit.length < PLAYER_COLLIDER_RADIUS + GROUNDED_HEIGHT && (hit.normalHit?.y ?? 0) >= GROUNDED_ANGLE_Y_LEN

var groundCaster: Entity;
var groundRayCaster: Entity;
var groundHitTick = 0;
var groundRayHitTick = 0;

export function initGroundRaycast() {
    engine.addSystem(recordGroundState, 100000 - 2);

    groundCaster = engine.addEntity();
    Transform.create(groundCaster, { parent: engine.PlayerEntity, position: { x: 0, y: PLAYER_COLLIDER_RADIUS, z: 0 } });

    raycastSystem.registerGlobalDirectionRaycast({
        entity: groundCaster,
        opts: {
            maxDistance: PLAYER_COLLIDER_RADIUS + GROUND_CAST_RANGE,
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            continuous: true,
            collisionMask: ColliderLayer.CL_PHYSICS,
            shape: RaycastShape.RS_AVATAR,
            includeWorld: true,
            direction: Vector3.Down()
        }
    },
        (hit) => {
            Vector3.copyFrom(VEC3_ZERO, groundNormal);
            groundDistance = playerPosition.y;

            if (hit.hits.some((hit) => {
                Vector3.copyFrom(hit.normalHit ?? VEC3_ZERO, groundNormal);
                groundDistance = Math.min(hit.length - PLAYER_COLLIDER_RADIUS, playerPosition.y);
                return groundedHit(hit);
            })) {
                groundHitTick = tick;
            }
        }
    )

    // Point-ray fallback for the grounded test: the capsule cast's first hit
    // can be a graze against the corner of the step behind (its normal comes
    // from the capsule surface, so a side contact reads near-horizontal and
    // unwalkable, masking the flat tread underfoot). A center ray reports the
    // true face normal below the player and can't graze. It reaches as far as the
    // capsule cast, so the edge test (centreSupported) can tell what lies under the centre.
    groundRayCaster = engine.addEntity();
    Transform.create(groundRayCaster, { parent: engine.PlayerEntity, position: { x: 0, y: PLAYER_COLLIDER_RADIUS, z: 0 } });

    raycastSystem.registerGlobalDirectionRaycast({
        entity: groundRayCaster,
        opts: {
            maxDistance: PLAYER_COLLIDER_RADIUS + GROUND_CAST_RANGE,
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            continuous: true,
            collisionMask: ColliderLayer.CL_PHYSICS,
            shape: RaycastShape.RS_RAY,
            includeWorld: true,
            direction: Vector3.Down()
        }
    },
        (hit) => {
            groundRayDistance = Infinity;
            Vector3.copyFrom(VEC3_ZERO, groundRayNormal);
            for (const h of hit.hits) {
                groundRayDistance = h.length - PLAYER_COLLIDER_RADIUS;
                Vector3.copyFrom(h.normalHit ?? VEC3_ZERO, groundRayNormal);
            }
            if (hit.hits.some(groundedHit)) {
                groundRayHitTick = tick;
            }
        }
    )
}

function recordGroundState() {
    prevGrounded = grounded;
    grounded = ((groundHitTick == tick) || (groundRayHitTick == tick) || (playerPosition.y < 0.01));
    if (playerPosition.y < 0.01) {
        Vector3.copyFrom(VEC3_UP, groundNormal);
    }
    if (!grounded) {
        Vector3.copyFrom(VEC3_ZERO, groundNormal);
    }
}

export function setGrounded(g: boolean) {
    grounded = g;
}

// height of the ground the avatar stood on at the end of the previous tick (NaN: not grounded). On an edge it is the
// edge's top, where the capsule touches it.
var prevGroundY = NaN;
// slope (tangent of the angle) of the ground under the centre at the end of the previous tick
var prevGroundSlope = 0;
const slopeOf = (n: Vector3) => (n.y > 0 ? Math.sqrt(Math.max(0, 1 - n.y * n.y)) / n.y : Infinity);

export function recordGroundForNextTick() {
    prevGroundY = !grounded ? NaN
        : centreOnGround() ? playerPosition.y - groundRayDistance
        : playerPosition.y - Math.max(0, groundDistance)
            + (groundNormal.y >= GROUNDED_ANGLE_Y_LEN ? PLAYER_COLLIDER_RADIUS * (1 - groundNormal.y) : 0);
    prevGroundSlope = slopeOf(groundRayNormal);
}

export function centreOnGround(): boolean {
    return groundRayDistance !== Infinity && groundRayNormal.y >= GROUNDED_ANGLE_Y_LEN
        && groundRayDistance - Math.max(0, groundDistance) <= GROUND_SNAP_STEP_DROP;
}

// False on an edge, where only the capsule's rim rests on the corner of a ledge, a riser or a steep crest.
export function centreSupported(): boolean {
    return centreOnGround() && stepDropAllowsSnap();
}

// A drop from the ground stood on last tick larger than GROUND_SNAP_STEP_DROP, plus what the steeper slope (then or
// now) drops over the distance moved, is a step edge.
function stepDropAllowsSnap(): boolean {
    if (Number.isNaN(prevGroundY)) return true;
    if (groundRayDistance === Infinity) return false;
    const slope = Math.max(slopeOf(groundRayNormal), prevGroundSlope);
    const travelled = Math.hypot(playerPosition.x - prevPlayerPosition.x, playerPosition.z - prevPlayerPosition.z);
    return prevGroundY - (playerPosition.y - groundRayDistance) <= GROUND_SNAP_STEP_DROP + travelled * slope;
}
