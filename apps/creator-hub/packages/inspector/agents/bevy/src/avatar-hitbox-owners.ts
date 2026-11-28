/**
 * Which NPC each hitbox drawn by avatar-hitboxes.ts stands for. Kept apart so the gizmo's pick
 * handler can read it without importing avatar-hitboxes.ts, which imports the gizmo.
 */

const ownerOf = new Map<number, number>();

export function registerAvatarHitbox(hitbox: number, entity: number): void {
  ownerOf.set(hitbox, entity);
}

export function clearAvatarHitboxOwners(): void {
  ownerOf.clear();
}

/** The NPC entity a hit hitbox stands for, or null when `id` is not a hitbox. */
export function getAvatarHitboxOwner(id: number): number | null {
  return ownerOf.get(id) ?? null;
}
