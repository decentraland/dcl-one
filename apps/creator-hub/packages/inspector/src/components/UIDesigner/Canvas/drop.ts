import { analytics, Event } from '../../../lib/logic/analytics';
import { spliceAddChild, spliceInsertComponent } from '../code/store';
import { acceptsElementWrite } from '../code/store-nodes';
import type { DropPoint } from '../code/store-splices';
import type { CodeUINode } from '../code/types';
import { UI_DESIGNER_DND_TYPE, type UIDesignerDragItem } from '../shared/dnd';
import type { UINodeType } from '../shared/tree-model';

export function applyCanvasDrop(item: UIDesignerDragItem, entity: number, pos?: DropPoint): void {
  if (item.source === 'palette') {
    void spliceAddChild(entity, item.type as UINodeType, item.preset, pos);
  } else if (item.source === 'component') {
    void spliceInsertComponent(entity, item.name, pos);
    analytics.track(Event.NEST_UI_COMPONENT, {});
  }
}

/** Whether the drop outline belongs on `node`: the pointer is over it and a drop there would write into it. */
export const showsDropOutline = (over: boolean, node: CodeUINode): boolean =>
  over && acceptsElementWrite(node);

type DragMonitor = { isDragging(): boolean; getItemType(): unknown; getItem(): unknown };

/** Whether a palette item is being dragged right now (a `useDragLayer` collector). */
export const isPaletteDrag = (monitor: DragMonitor): boolean =>
  monitor.isDragging() &&
  monitor.getItemType() === UI_DESIGNER_DND_TYPE &&
  (monitor.getItem() as UIDesignerDragItem | null)?.source === 'palette';
