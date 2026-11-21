import React, { useRef } from 'react';
import { readFileSync } from 'fs';
import { join } from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import type { Entity } from '@dcl/ecs';

import type * as StoreNodes from '../../code/store-nodes';
import { WidgetPicker } from './WidgetPicker';

type Node = { entity: number; children: Node[] };

const mocks = vi.hoisted(() => ({
  spliceAddChild: vi.fn(),
  spliceSetRootChild: vi.fn(),
  dispatch: vi.fn(),
  snapshot: { source: '', parsed: null as null | { root: Node } },
}));

vi.mock('../../code/store', async () => {
  const nodes = await vi.importActual<typeof StoreNodes>('../../code/store-nodes');
  return {
    findCodeNode: nodes.findCodeNode,
    getSnapshot: () => mocks.snapshot,
    spliceAddChild: mocks.spliceAddChild,
    spliceSetRootChild: mocks.spliceSetRootChild,
  };
});

vi.mock('../../../../redux/hooks', () => ({ useAppDispatch: () => mocks.dispatch }));

const node = (entity: number, children: Node[] = []): Node => ({ entity, children });

function Harness({
  parent = 2 as unknown as Entity,
  offset,
  onDismiss,
}: {
  parent?: Entity | null;
  offset?: number;
  onDismiss: () => void;
}) {
  const chipRef = useRef<HTMLSpanElement>(null);
  return (
    <>
      <button
        type="button"
        id="before"
      >
        Before
      </button>
      <button
        type="button"
        id="opener"
      >
        <span ref={chipRef}>+</span>
      </button>
      <button
        type="button"
        id="after"
      >
        After
      </button>
      <WidgetPicker
        parent={parent}
        anchorRef={chipRef}
        offset={offset}
        onDismiss={onDismiss}
      />
    </>
  );
}

const menu = () => document.querySelector<HTMLElement>('.ui-designer-widget-picker')!;
const search = () => menu().querySelector<HTMLInputElement>('input')!;
const options = () => [...menu().querySelectorAll<HTMLElement>('[role="option"]')];
const optionLabels = () => options().map(o => o.textContent);
const activeOption = () =>
  document.getElementById(search().getAttribute('aria-activedescendant') ?? '')?.textContent;
const opener = () => document.getElementById('opener')!;

const press = (key: string, init: KeyboardEventInit = {}) => {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  act(() => {
    search().dispatchEvent(event);
  });
  return event;
};

const typeQuery = async (value: string) => {
  fireEvent.input(search(), { target: { value } });
  await act(() => new Promise(resolve => setTimeout(resolve, 0)));
};

let onDismiss: ReturnType<typeof vi.fn<() => void>>;

const open = (props: { parent?: Entity | null; offset?: number } = {}) =>
  render(
    <Harness
      {...props}
      onDismiss={onDismiss}
    />,
  );

beforeEach(() => {
  onDismiss = vi.fn<() => void>();
  mocks.spliceAddChild.mockReset();
  mocks.spliceSetRootChild.mockReset();
  mocks.dispatch.mockReset();
  mocks.snapshot = { source: 'before', parsed: { root: node(1, [node(2)]) } };
});

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

describe('when the Add Node menu opens', () => {
  it('should be a dialog named Add Node holding the search, a divider and the node types', () => {
    open();

    expect(menu().getAttribute('role')).toBe('dialog');
    expect(menu().getAttribute('aria-label')).toBe('Add Node');
    expect(search().getAttribute('role')).toBe('searchbox');
    expect(search().getAttribute('type')).toBe('text');
    expect(search().getAttribute('aria-label')).toBe('Search node type');
    expect(search().getAttribute('placeholder')).toBe('Search node type');
    expect(menu().querySelectorAll('[role="separator"]')).toHaveLength(1);
    const listbox = menu().querySelector('[role="listbox"]')!;
    expect(listbox.getAttribute('aria-label')).toBe('Node types');
    expect(search().getAttribute('aria-controls')).toBe(listbox.id);
  });

  it('should group the seven node types under their categories', () => {
    open();
    const groups = [...menu().querySelectorAll('[role="group"]')];

    expect(
      groups.map(g => document.getElementById(g.getAttribute('aria-labelledby')!)?.textContent),
    ).toEqual(['Containers', 'Text', 'Input']);
    expect(optionLabels()).toEqual([
      'Full Screen',
      'Container',
      'Image',
      'Label',
      'Button',
      'Input',
      'Dropdown',
    ]);
  });

  it('should focus the search with no node type active', () => {
    open();

    expect(document.activeElement).toBe(search());
    expect(search().hasAttribute('aria-activedescendant')).toBe(false);
    expect(options().some(o => o.getAttribute('aria-selected') === 'true')).toBe(false);
  });

  it('should keep the node types out of the Tab order', () => {
    open();

    expect(options().some(o => o.hasAttribute('tabindex'))).toBe(false);
  });

  it('should sit 4px below its anchor by default and at the offset it is given', () => {
    open();
    expect(menu().style.top).toBe('4px');
    cleanup();

    open({ offset: 8 });
    expect(menu().style.top).toBe('8px');
  });
});

describe('when typing in the search', () => {
  it('should list the node types whose name, category or keyword matches', async () => {
    open();
    await typeQuery('Text');

    expect(optionLabels()).toEqual(['Image', 'Label', 'Button', 'Input']);
  });

  it('should reset the active node type', async () => {
    open();
    press('ArrowDown');
    await typeQuery('box');

    expect(search().hasAttribute('aria-activedescendant')).toBe(false);
    expect(optionLabels()).toEqual(['Container']);
  });

  describe('and nothing matches', () => {
    it('should empty the list under the search and the divider, with no message', async () => {
      open();
      await typeQuery('zzz');

      expect(options()).toHaveLength(0);
      expect(menu().querySelectorAll('[role="group"]')).toHaveLength(0);
      expect(menu().querySelector('[role="separator"]')).not.toBeNull();
      expect(menu().querySelector('[role="listbox"]')!.textContent).toBe('');
      expect(menu().textContent).not.toContain('No widgets match');
    });
  });

  it('should offer a pointer-only Clear search button that empties the field and keeps focus', async () => {
    open();
    await typeQuery('lab');
    const clear = menu().querySelector<HTMLButtonElement>('button[aria-label="Clear search"]')!;

    expect(clear.tabIndex).toBe(-1);
    act(() => clear.focus());
    fireEvent.click(clear);
    await act(() => new Promise(resolve => setTimeout(resolve, 0)));

    expect(search().value).toBe('');
    expect(document.activeElement).toBe(search());
    expect(menu().querySelector('button[aria-label="Clear search"]')).toBeNull();
    expect(optionLabels()).toHaveLength(7);
  });
});

describe('when moving through the node types with the arrows', () => {
  it('should activate them in order, clamped at both ends, with focus staying in the search', () => {
    open();

    expect(press('ArrowDown').defaultPrevented).toBe(true);
    expect(activeOption()).toBe('Full Screen');
    press('ArrowUp');
    expect(activeOption()).toBe('Full Screen');
    press('ArrowDown');
    press('ArrowDown');
    press('ArrowDown');
    expect(activeOption()).toBe('Label');
    expect(press('ArrowUp').defaultPrevented).toBe(true);
    expect(activeOption()).toBe('Image');
    for (let i = 0; i < 10; i++) press('ArrowDown');
    expect(activeOption()).toBe('Dropdown');
    expect(document.activeElement).toBe(search());
  });

  it('should mark only the active node type selected', () => {
    open();
    press('ArrowDown');

    expect(options().filter(o => o.getAttribute('aria-selected') === 'true')).toEqual([
      options()[0],
    ]);
  });
});

describe('when a node type is added', () => {
  const row = () => {
    const tree = document.createElement('div');
    tree.className = 'ui-designer-nodetree';
    const item = document.createElement('div');
    item.setAttribute('role', 'treeitem');
    item.tabIndex = -1;
    item.dataset.treeId = '3';
    tree.appendChild(item);
    document.body.appendChild(tree);
    return item;
  };
  const addsChild = () =>
    mocks.spliceAddChild.mockImplementation(async () => {
      mocks.snapshot = { source: 'after', parsed: { root: node(1, [node(2, [node(3)])]) } };
    });

  it('should add the first match on Enter when none is active', async () => {
    addsChild();
    open();
    await typeQuery('lab');
    press('Enter');

    expect(mocks.spliceAddChild).toHaveBeenCalledWith(2, 'Label', undefined);
    expect(onDismiss).toHaveBeenCalled();
  });

  it('should add the match for what was just typed when Enter follows at once', () => {
    addsChild();
    open();
    fireEvent.input(search(), { target: { value: 'lab' } });
    press('Enter');

    expect(mocks.spliceAddChild).toHaveBeenCalledWith(2, 'Label', undefined);
  });

  it('should add the active node type on Enter', () => {
    addsChild();
    open();
    press('ArrowDown');
    press('ArrowDown');
    press('Enter');

    expect(mocks.spliceAddChild).toHaveBeenCalledWith(2, 'UiEntity', undefined);
  });

  it('should do nothing on Enter when nothing matches', async () => {
    open();
    await typeQuery('zzz');
    press('Enter');

    expect(mocks.spliceAddChild).not.toHaveBeenCalled();
    expect(onDismiss).not.toHaveBeenCalled();
  });

  it('should add the clicked node type, keeping focus in the search on press', () => {
    addsChild();
    open();
    const image = options()[2];

    expect(fireEvent.mouseDown(image)).toBe(false);
    fireEvent.click(image);

    expect(mocks.spliceAddChild).toHaveBeenCalledWith(2, 'UiEntity', 'image');
  });

  it('should select the new node and focus its NODES tree row once the source reparses', async () => {
    addsChild();
    const treeRow = row();
    open();
    press('Enter');

    await waitFor(() => expect(document.activeElement).toBe(treeRow));
    expect(mocks.dispatch).toHaveBeenCalledWith(expect.objectContaining({ payload: { node: 3 } }));
  });

  it("should write the GUI's root element when there is no parent, and select it", async () => {
    mocks.spliceSetRootChild.mockImplementation(async () => {
      mocks.snapshot = { source: 'after', parsed: { root: node(3) } };
    });
    const treeRow = row();
    open({ parent: null });
    press('Enter');

    expect(mocks.spliceSetRootChild).toHaveBeenCalledWith('UiEntity', 'fullscreen');
    await waitFor(() => expect(document.activeElement).toBe(treeRow));
    expect(mocks.dispatch).toHaveBeenCalledWith(expect.objectContaining({ payload: { node: 3 } }));
  });

  it('should select nothing when the source did not change', async () => {
    mocks.spliceAddChild.mockResolvedValue(undefined);
    open();
    press('Enter');
    await act(() => new Promise(resolve => setTimeout(resolve, 0)));

    expect(mocks.dispatch).not.toHaveBeenCalled();
  });
});

describe('when the menu is closed from the keyboard', () => {
  it('should close at once on Escape, whatever the search holds, and return focus to the opener', async () => {
    const onDocumentKey = vi.fn();
    open();
    await typeQuery('Text');
    document.addEventListener('keydown', onDocumentKey);
    const event = press('Escape');
    document.removeEventListener('keydown', onDocumentKey);

    expect(onDismiss).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(opener());
    expect(event.defaultPrevented).toBe(true);
    expect(onDocumentKey).not.toHaveBeenCalled();
  });

  it.each([[{}], [{ shiftKey: true }]])(
    'should close on Tab %j and hand focus to the opener for the browser to move on from',
    init => {
      open();
      const event = press('Tab', init);

      expect(onDismiss).toHaveBeenCalledTimes(1);
      expect(document.activeElement).toBe(opener());
      expect(event.defaultPrevented).toBe(false);
    },
  );
});

describe('when the pointer goes down outside the menu', () => {
  it('should close without moving focus back to the opener', () => {
    open();
    fireEvent.mouseDown(document.getElementById('after')!);

    expect(onDismiss).toHaveBeenCalled();
    expect(document.activeElement).not.toBe(opener());
  });
});

describe('when the stylesheet draws the clear button and the entries', () => {
  const stylesheet = readFileSync(join(__dirname, 'WidgetPicker.css'), 'utf8');
  const rule = (selector: string) =>
    Array.from(stylesheet.matchAll(/([^{}]+)\{([^}]*)\}/g))
      .find(([, sel]) => sel.replace(/\s+/g, ' ').trim() === selector)?.[2]
      .replace(/\s+/g, ' ');

  it('should size the Close glyph to two thirds of its 16px chip', () => {
    expect(rule('.ui-designer-widget-picker .ClearSearch')).toContain(
      'font-size: calc(16px * 2 / 3);',
    );
  });

  it('should give the pointer-only clear button no keyboard focus rules', () => {
    expect(stylesheet).not.toMatch(/ClearSearch:focus-visible/);
  });

  it('should drop the tap delay on the entries', () => {
    expect(rule('.ui-designer-widget-picker-row')).toContain('touch-action: manipulation;');
  });
});
