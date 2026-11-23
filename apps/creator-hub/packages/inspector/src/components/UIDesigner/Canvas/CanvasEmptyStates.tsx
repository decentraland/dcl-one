import React, { useRef, useState } from 'react';
import { useDrop } from 'react-dnd';
import { cx } from '../../../lib/utils/cx';

import { Button } from '../../Button';
import { Plus } from '../../Icons/figma';
import { createRoot, spliceSetRootChild } from '../code/store';
import { EmptyGuiGlyph, EmptyState, EmptyStateChip, NoGuiGlyph } from '../EmptyState';
import { WidgetPicker } from '../LeftPanel/WidgetPicker';
import { UI_DESIGNER_DND_TYPE, type UIDesignerDragItem } from '../shared/dnd';

/** The canvas when the scene has no GUI: first-run guidance and a New GUI button. */
export const NoGuiState: React.FC = () => (
  <div className="ui-designer-canvas-empty ui-designer-canvas-empty--no-gui">
    <EmptyState
      variant="canvas"
      icon={<NoGuiGlyph />}
      title="Start building your UI"
      message={
        <>
          Click the <EmptyStateChip tone="muted">GUIs +</EmptyStateChip> button in the left panel to
          add UI elements. Then, select elements such as <strong>text, buttons, and images</strong>{' '}
          from the <EmptyStateChip tone="muted">Nodes</EmptyStateChip> section to design what
          players will see in your scene.
        </>
      }
      action={
        <Button
          variant="secondary"
          size="lg"
          onClick={() => void createRoot()}
        >
          <Plus fontSize="inherit" />
          New GUI
        </Button>
      }
    />
  </div>
);

/** The canvas when the selected GUI has no node: an Add Node opener and a drop target for palette items. */
export const EmptyGuiState: React.FC = () => {
  const addRef = useRef<HTMLButtonElement>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [{ isOver }, drop] = useDrop<UIDesignerDragItem, unknown, { isOver: boolean }>(
    () => ({
      accept: UI_DESIGNER_DND_TYPE,
      drop: (item, monitor) => {
        if (monitor.didDrop()) return;
        if (item.source === 'palette') void spliceSetRootChild(item.type, item.preset);
      },
      collect: monitor => ({ isOver: monitor.isOver({ shallow: true }) }),
    }),
    [],
  );
  return (
    <div
      ref={drop}
      className={cx('ui-designer-canvas-empty ui-designer-canvas-empty--empty-gui', {
        over: isOver,
      })}
    >
      <EmptyState
        variant="canvas"
        icon={<EmptyGuiGlyph />}
        title="This GUI is empty"
        message={
          <>
            Click the <EmptyStateChip tone="strong">Nodes +</EmptyStateChip> button in the left
            panel to add your first UI element.
          </>
        }
        action={
          <Button
            ref={addRef}
            variant="secondary"
            size="lg"
            aria-haspopup="dialog"
            aria-expanded={pickerOpen}
            onClick={() => setPickerOpen(true)}
          >
            <Plus fontSize="inherit" />
            Add Node
          </Button>
        }
      />
      {pickerOpen ? (
        <WidgetPicker
          parent={null}
          anchorRef={addRef}
          onDismiss={() => setPickerOpen(false)}
        />
      ) : null}
    </div>
  );
};
