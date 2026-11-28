import { engine, Transform, MeshCollider, ColliderLayer } from '@dcl/sdk/ecs';
import type { Entity } from '@dcl/sdk/ecs';
import { Vector3 } from '@dcl/sdk/math';
import type { AvatarHitbox } from '@dcl/inspector-bevy-protocol';

import { getSceneOffset } from './gizmo';
import { clearAvatarHitboxOwners, registerAvatarHitbox } from './avatar-hitbox-owners';

/**
 * The engine collides scene raycasts with colliders only, and gives a scene's NPCs none, so a
 * viewport click passes through them. Put an invisible pointer box over each NPC the inspector
 * reports; the pick handler maps a hit on it back to the NPC's entity.
 */

// A standing avatar: about 1.8 m tall from the feet, which sit on the entity's origin.
const HITBOX_WIDTH = 0.8;
const HITBOX_HEIGHT = 1.9;

let roots: Entity[] = [];

function clear(): void {
  for (const root of roots) engine.removeEntityWithChildren(root);
  roots = [];
  clearAvatarHitboxOwners();
}

/** Replace the NPC hitboxes with `hitboxes` (empty clears them). */
export function setAvatarHitboxes(hitboxes: AvatarHitbox[]): void {
  clear();
  const offset = getSceneOffset();
  for (const { entity, position, rotation, scale } of hitboxes) {
    const root = engine.addEntity();
    Transform.create(root, {
      position: Vector3.add(Vector3.create(position.x, position.y, position.z), offset),
      rotation,
      scale,
    });
    const hitbox = engine.addEntity();
    Transform.create(hitbox, {
      position: Vector3.create(0, HITBOX_HEIGHT / 2, 0),
      scale: Vector3.create(HITBOX_WIDTH, HITBOX_HEIGHT, HITBOX_WIDTH),
      parent: root,
    });
    MeshCollider.setBox(hitbox, ColliderLayer.CL_POINTER);
    registerAvatarHitbox(hitbox as number, entity);
    roots.push(root);
  }
}
