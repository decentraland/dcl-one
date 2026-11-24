import { analytics, Event } from '../../../lib/logic/analytics';

/**
 * "Add New Action" appends a row with no type, so the add is complete, and its type known, only
 * when the creator picks the row's first type. Retyping an existing action is not an add.
 */
export function trackActionTypeSelected(
  previousType: string,
  actionType: string,
  itemPath?: string,
) {
  if (previousType) return;
  analytics.track(Event.ADD_SMART_ITEM_ACTION, { actionType, itemPath });
}
