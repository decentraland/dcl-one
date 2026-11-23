import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Entity } from '@dcl/ecs';

import { useAppDispatch } from '../../../../redux/hooks';
import { selectNode } from '../../../../redux/ui-designer';
import { Close, Search } from '../../../Icons/figma';
import { TextField } from '../../../ui/TextField';
import { usePopoverPosition } from '../../../ui/usePopoverPosition';
import { findCodeNode, getSnapshot, spliceAddChild, spliceSetRootChild } from '../../code/store';
import { matchWidgets, type WidgetDef } from '../../shared/widget-catalog';

import './WidgetPicker.css';

const WIDTH = 220;

interface WidgetPickerProps {
  /** The node the new widget is appended to; `null` writes it as the GUI's root element. */
  parent: Entity | null;
  anchorRef: React.RefObject<HTMLElement>;
  /** Gap in px between the anchor's bottom edge and the menu. */
  offset?: number;
  onDismiss: () => void;
}

const optionId = (item: WidgetDef) => `widget-picker-${item.id}`;

async function addWidget(parent: Entity | null, item: WidgetDef): Promise<Entity | undefined> {
  const before = getSnapshot().source;
  if (parent === null) await spliceSetRootChild(item.type, item.preset);
  else await spliceAddChild(parent as unknown as number, item.type, item.preset);
  const { source, parsed } = getSnapshot();
  if (source === before) return undefined;
  if (parent === null) return parsed?.root?.entity;
  return findCodeNode(parsed?.root, parent as unknown as number)?.children.at(-1)?.entity;
}

function focusTreeRow(entity: Entity, frames = 10) {
  const row = document.querySelector<HTMLElement>(
    `.ui-designer-nodetree [role="treeitem"][data-tree-id="${String(entity)}"]`,
  );
  if (row) row.focus();
  else if (frames > 0) requestAnimationFrame(() => focusTreeRow(entity, frames - 1));
}

/** The Add Node menu: a search over the widget catalog that appends the picked widget under `parent`, then selects and focuses the new node. */
export const WidgetPicker: React.FC<WidgetPickerProps> = ({
  parent,
  anchorRef,
  offset = 4,
  onDismiss,
}) => {
  const dispatch = useAppDispatch();
  const popoverRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const [query, setQuery] = useState('');
  const [active, setActive] = useState<number | null>(null);
  const pos = usePopoverPosition({
    anchorRef,
    popoverRef,
    open: true,
    onDismiss,
    width: WIDTH,
    gap: offset,
  });

  const categories = useMemo(() => matchWidgets(query), [query]);
  const items = useMemo(() => categories.flatMap(c => c.items), [categories]);
  const activeItem = active === null ? undefined : items[active];

  useEffect(() => {
    if (activeItem)
      document.getElementById(optionId(activeItem))?.scrollIntoView({ block: 'nearest' });
  }, [activeItem]);

  const changeQuery = (next: string) => {
    setQuery(next);
    setActive(null);
  };

  const add = (item: WidgetDef) => {
    onDismiss();
    void addWidget(parent, item).then(entity => {
      if (entity === undefined) return;
      dispatch(selectNode({ node: entity }));
      focusTreeRow(entity);
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        e.stopPropagation();
        anchorRef.current?.closest('button')?.focus();
        onDismiss();
        return;
      case 'Tab':
        anchorRef.current?.closest('button')?.focus();
        onDismiss();
        return;
      case 'ArrowDown':
      case 'ArrowUp': {
        e.preventDefault();
        e.stopPropagation();
        if (items.length === 0) return;
        const step = e.key === 'ArrowDown' ? 1 : -1;
        setActive(at => (at === null ? 0 : Math.min(Math.max(at + step, 0), items.length - 1)));
        return;
      }
      case 'Enter': {
        e.preventDefault();
        e.stopPropagation();
        const item = activeItem ?? items[0];
        if (item) add(item);
      }
    }
  };

  return createPortal(
    <div
      ref={popoverRef}
      className="ui-designer-widget-picker"
      role="dialog"
      aria-label="Add Node"
      style={{ position: 'fixed', top: pos.top, left: pos.left, width: WIDTH }}
      onKeyDownCapture={handleKeyDown}
    >
      <div className="ui-designer-widget-picker-search">
        <TextField
          ref={searchRef}
          role="searchbox"
          autoComplete="off"
          autoFocus
          aria-label="Search node type"
          placeholder="Search node type"
          aria-controls={listId}
          aria-activedescendant={activeItem ? optionId(activeItem) : undefined}
          value={query}
          onInput={e => changeQuery(e.currentTarget.value)}
          leftIcon={<Search fontSize="small" />}
          rightIcon={
            query ? (
              <button
                type="button"
                className="ClearSearch"
                aria-label="Clear search"
                tabIndex={-1}
                onClick={() => {
                  changeQuery('');
                  searchRef.current?.focus();
                }}
              >
                <Close fontSize="inherit" />
              </button>
            ) : undefined
          }
        />
      </div>
      <div
        className="ui-designer-widget-picker-divider"
        role="separator"
      />
      <div
        id={listId}
        className="ui-designer-widget-picker-list"
        role="listbox"
        aria-label="Node types"
      >
        {categories.map(c => (
          <div
            key={c.category}
            role="group"
            aria-labelledby={`${listId}-${c.category}`}
          >
            <div
              id={`${listId}-${c.category}`}
              className="ui-designer-widget-picker-category"
            >
              {c.category}
            </div>
            {c.items.map(item => (
              <div
                key={item.id}
                id={optionId(item)}
                className="ui-designer-widget-picker-row"
                role="option"
                aria-selected={item === activeItem}
                onMouseDown={e => e.preventDefault()}
                onClick={() => add(item)}
              >
                <span
                  className="ui-designer-widget-picker-icon"
                  aria-hidden="true"
                >
                  <item.Icon fontSize="small" />
                </span>
                <span>{item.label}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>,
    document.body,
  );
};

export default WidgetPicker;
