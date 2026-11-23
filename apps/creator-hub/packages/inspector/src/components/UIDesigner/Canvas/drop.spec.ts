import { readFileSync } from 'fs';
import { join } from 'path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type * as AnalyticsModule from '../../../lib/logic/analytics';
import { Event } from '../../../lib/logic/analytics';
import type { UIDesignerDragItem } from '../shared/dnd';
import type { CodeUINode } from '../code/types';
import { UI_DESIGNER_DND_TYPE } from '../shared/dnd';
import { applyCanvasDrop, isPaletteDrag, showsDropOutline } from './drop';

const mocks = vi.hoisted(() => ({
  track: vi.fn(),
  spliceAddChild: vi.fn(),
  spliceInsertComponent: vi.fn(),
}));

vi.mock('../../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

vi.mock('../code/store', () => ({
  spliceAddChild: mocks.spliceAddChild,
  spliceInsertComponent: mocks.spliceInsertComponent,
}));

beforeEach(() => {
  mocks.track.mockClear();
  mocks.spliceAddChild.mockClear();
  mocks.spliceInsertComponent.mockClear();
});

describe('applyCanvasDrop', () => {
  it('should nest and track when a GUI component is dropped', () => {
    const pos = { top: 40, left: 60 };
    applyCanvasDrop({ source: 'component', name: 'Sidebar' } as UIDesignerDragItem, 5, pos);

    expect(mocks.spliceInsertComponent).toHaveBeenCalledWith(5, 'Sidebar', pos);
    expect(mocks.track).toHaveBeenCalledWith(Event.NEST_UI_COMPONENT, {});
  });

  it('should not track when a palette widget is dropped', () => {
    applyCanvasDrop(
      { source: 'palette', type: 'UiEntity', preset: undefined } as UIDesignerDragItem,
      5,
    );

    expect(mocks.spliceAddChild).toHaveBeenCalled();
    expect(mocks.track).not.toHaveBeenCalled();
  });
});

describe('showsDropOutline', () => {
  const node = (over: Partial<CodeUINode> = {}) =>
    ({
      entity: 1,
      type: 'UiEntity',
      name: 'UiEntity',
      children: [],
      ...over,
    }) as unknown as CodeUINode;

  it.each([
    ['a container', node()],
    ['a leaf, which takes the item as a child', node({ type: 'Label', name: 'Label' })],
  ])('should outline %s the pointer is over', (_, target) => {
    expect(showsDropOutline(true, target)).toBe(true);
  });

  it('should clear the outline once the pointer is no longer over the node', () => {
    expect(showsDropOutline(false, node())).toBe(false);
  });

  it.each([
    ['dynamic props', { dynamicProps: true }],
    ['a component instance', { componentRef: { name: 'Card' } }],
    ['opaque code', { opaque: true }],
    ['a platform variant', { platformVariant: true }],
  ])('should not outline a node with %s, since the drop writes nothing there', (_, flags) => {
    expect(showsDropOutline(true, node(flags as Partial<CodeUINode>))).toBe(false);
  });
});

describe('the drop outline look', () => {
  const css = readFileSync(join(__dirname, 'Canvas.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const rules = Array.from(css.matchAll(/([^{}]+)\{([^{}]*)\}/g), ([, sel, body]) => ({
    selector: sel.trim(),
    body: body.replace(/\s+/g, ' ').trim(),
  }));
  const index = (selector: string) => rules.findIndex(r => r.selector === selector);
  const dropOver = rules[index('.ui-designer-canvas-node.drop-over')]?.body ?? '';

  it('should draw a 1px Blue Light 1 inset line, held at 1px on screen at any zoom', () => {
    expect(dropOver).toContain(
      'box-shadow: inset 0 0 0 calc(1px / var(--uid-scale, 1)) var(--palette-info-light);',
    );
  });

  it('should drop the dashed outline and the fill', () => {
    expect(dropOver).toContain('outline: none;');
    expect(dropOver).not.toMatch(/dashed|background/);
  });

  it("should win over the node's own locator, hover and selection outlines", () => {
    const at = index('.ui-designer-canvas-node.drop-over');
    expect(at).toBeGreaterThan(index('.ui-designer-canvas-node'));
    expect(at).toBeGreaterThan(index('.ui-designer-canvas-node:hover'));
    expect(at).toBeGreaterThan(index('.ui-designer-canvas-node.selected'));
  });
});

describe('isPaletteDrag', () => {
  const monitor = (dragging: boolean, type: unknown, item: unknown) => ({
    isDragging: () => dragging,
    getItemType: () => type,
    getItem: () => item,
  });

  it('should be true while a palette item is being dragged', () => {
    expect(
      isPaletteDrag(monitor(true, UI_DESIGNER_DND_TYPE, { source: 'palette', type: 'Label' })),
    ).toBe(true);
  });

  it.each([
    ['once the drag ends or is cancelled', monitor(false, null, null)],
    [
      'for a GUI dragged in as a component',
      monitor(true, UI_DESIGNER_DND_TYPE, { source: 'component', name: 'Card' }),
    ],
    ['for any other drag type', monitor(true, 'ui-designer-tree', { source: 'palette' })],
  ])('should be false %s', (_, m) => {
    expect(isPaletteDrag(m)).toBe(false);
  });
});

describe('the selection look during a palette drag', () => {
  const css = readFileSync(join(__dirname, 'Canvas.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const body = (selector: string) =>
    Array.from(css.matchAll(/([^{}]+)\{([^{}]*)\}/g))
      .find(([, sel]) => sel.trim() === selector)?.[2]
      .replace(/\s+/g, ' ');

  it('should hide the selected node ring under the palette-dragging canvas', () => {
    expect(
      body('.ui-designer-canvas-viewport.palette-dragging .ui-designer-canvas-node.selected'),
    ).toContain('outline-color: transparent;');
  });

  it('should hide the resize handles under the palette-dragging canvas', () => {
    expect(
      body('.ui-designer-canvas-viewport.palette-dragging .ui-designer-resize-handle'),
    ).toContain('display: none;');
  });

  it("should hide the selected node's action toolbar under the palette-dragging canvas", () => {
    expect(
      body('.ui-designer-canvas-viewport.palette-dragging .ui-designer-node-actions'),
    ).toContain('display: none;');
  });

  it('should leave the selection, move-drag and resize looks themselves unchanged', () => {
    expect(body('.ui-designer-canvas-node.selected')).toContain('outline-style: solid;');
    expect(body('.ui-designer-canvas-node.dragging')).toContain('outline-style: dashed;');
    expect(body('.ui-designer-canvas-node.resizing')).toContain('outline-style: solid;');
  });
});
