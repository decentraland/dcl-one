import type { Entity } from '@dcl/ecs';

import { EDITOR_ENTITIES } from '../sdk/tree';
import { analytics, type EntityActionSource, type Event } from './analytics';

/**
 * Tracks one user action over a set of entities. The root, player and camera are left out of the
 * count, so pressing Delete with only the scene root selected reports nothing.
 */
export function trackEntityAction(
  event: Event.DELETE_ENTITY | Event.DUPLICATE_ENTITY,
  entities: readonly Entity[],
  source: EntityActionSource,
) {
  const entitiesCount = entities.filter(entity => !EDITOR_ENTITIES.includes(entity)).length;
  if (entitiesCount === 0) return;
  analytics.track(event, { entitiesCount, source });
}
