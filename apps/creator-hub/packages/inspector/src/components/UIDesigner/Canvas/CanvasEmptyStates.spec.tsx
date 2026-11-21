import { readFileSync } from 'fs';
import { join } from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';

import { EmptyGuiState, NoGuiState } from './CanvasEmptyStates';

type DropSpec = {
  drop: (item: { source: string; type: string; preset?: string }, monitor: unknown) => void;
};

const mocks = vi.hoisted(() => ({
  createRoot: vi.fn(),
  spliceSetRootChild: vi.fn(),
  isOver: false,
  dropSpec: null as null | DropSpec,
  dropTarget: null as null | Element,
  picker: null as null | {
    parent: unknown;
    anchorRef: { current: unknown };
    onDismiss: () => void;
  },
}));

vi.mock('../code/store', () => ({
  createRoot: mocks.createRoot,
  spliceSetRootChild: mocks.spliceSetRootChild,
}));

vi.mock('react-dnd', () => ({
  useDrop: (spec: () => DropSpec) => {
    mocks.dropSpec = spec();
    return [{ isOver: mocks.isOver }, (el: Element | null) => (mocks.dropTarget = el)];
  },
}));

vi.mock('../LeftPanel/WidgetPicker', () => ({
  WidgetPicker: (props: NonNullable<typeof mocks.picker>) => {
    mocks.picker = props;
    return <div role="dialog" />;
  },
}));

const state = () => document.querySelector<HTMLElement>('.ui-designer-canvas-empty')!;
const emptyState = () => state().querySelector<HTMLElement>('.ui-designer-empty-state--canvas')!;
const heading = () => emptyState().querySelector('h2')!;
const body = () => emptyState().querySelector('.ui-designer-empty-state-message')!;
const chips = () =>
  [...emptyState().querySelectorAll<HTMLElement>('.ui-designer-empty-state-chip')].map(c => [
    c.textContent,
    c.classList.contains('ui-designer-empty-state-chip--strong') ? 'strong' : 'muted',
  ]);
const button = () => emptyState().querySelector<HTMLButtonElement>('button')!;

beforeEach(() => {
  mocks.createRoot.mockReset();
  mocks.spliceSetRootChild.mockReset();
  mocks.isOver = false;
  mocks.dropSpec = null;
  mocks.dropTarget = null;
  mocks.picker = null;
});

afterEach(cleanup);

describe('when the scene has no GUI', () => {
  it('should guide the author with the drawn copy and muted chips', () => {
    render(<NoGuiState />);

    expect(heading().textContent).toBe('Start building your UI');
    expect(body().textContent).toBe(
      'Click the GUIs + button in the left panel to add UI elements. Then, select elements such as text, buttons, and images from the Nodes section to design what players will see in your scene.',
    );
    expect(body().querySelector('strong')!.textContent).toBe('text, buttons, and images');
    expect(chips()).toEqual([
      ['GUIs +', 'muted'],
      ['Nodes', 'muted'],
    ]);
  });

  it('should draw its 91 × 50 glyph hidden from assistive technology', () => {
    render(<NoGuiState />);
    const svg = emptyState().querySelector('svg')!;

    expect([svg.getAttribute('width'), svg.getAttribute('height')]).toEqual(['91', '50']);
    expect(svg.closest('[aria-hidden="true"]')).not.toBeNull();
  });

  it('should create a GUI from its New GUI button', () => {
    render(<NoGuiState />);

    expect(button().textContent).toBe('New GUI');
    fireEvent.click(button());
    expect(mocks.createRoot).toHaveBeenCalledTimes(1);
  });
});

describe('when the selected GUI is empty', () => {
  it('should point at the NODES + with the drawn copy and a strong chip', () => {
    render(<EmptyGuiState />);

    expect(heading().textContent).toBe('This GUI is empty');
    expect(body().textContent).toBe(
      'Click the Nodes + button in the left panel to add your first UI element.',
    );
    expect(chips()).toEqual([['Nodes +', 'strong']]);
    expect(document.body.textContent).not.toContain('Drag a widget');
    expect(document.body.textContent).not.toContain('+ Add element');
  });

  it('should draw its 50 × 50 glyph', () => {
    render(<EmptyGuiState />);
    const svg = emptyState().querySelector('svg')!;

    expect([svg.getAttribute('width'), svg.getAttribute('height')]).toEqual(['50', '50']);
  });

  it('should open the Add Node menu from its Add Node button, anchored to it, writing the root', () => {
    render(<EmptyGuiState />);

    expect(button().textContent).toBe('Add Node');
    expect(button().getAttribute('aria-haspopup')).toBe('dialog');
    expect(button().getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(button());

    expect(button().getAttribute('aria-expanded')).toBe('true');
    expect(mocks.picker?.parent).toBeNull();
    expect(mocks.picker?.anchorRef.current).toBe(button());
    act(() => mocks.picker?.onDismiss());
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(button().getAttribute('aria-expanded')).toBe('false');
  });

  it('should write the root child when a palette item is dropped on it', () => {
    render(<EmptyGuiState />);
    mocks.dropSpec?.drop({ source: 'palette', type: 'Label' }, { didDrop: () => false });

    expect(mocks.spliceSetRootChild).toHaveBeenCalledWith('Label', undefined);
  });

  it('should mark itself as a drop target while a palette item is over it', () => {
    mocks.isOver = true;
    render(<EmptyGuiState />);

    expect(state().classList.contains('over')).toBe(true);
  });
});

describe('when a palette item is dragged over the canvas', () => {
  const css = (file: string) =>
    readFileSync(join(__dirname, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const rule = (file: string, selector: string) =>
    Array.from(css(file).matchAll(/([^{}]+)\{([^{}]*)\}/g))
      .find(([, sel]) => sel.trim() === selector)?.[2]
      .replace(/\s+/g, ' ');

  it('should make the whole empty-GUI area the drop target, not the content block', () => {
    render(<EmptyGuiState />);

    expect(mocks.dropTarget).toBe(state());
    expect(mocks.dropTarget).not.toBe(emptyState());
  });

  it('should stretch that area over the whole canvas viewport, so a drop anywhere lands', () => {
    expect(rule('Canvas.css', '.ui-designer-canvas-viewport')).toContain('position: relative;');
    const area = rule('Canvas.css', '.ui-designer-canvas-empty--empty-gui');
    expect(area).toContain('position: absolute;');
    expect(area).toContain('inset: 0;');
  });

  it('should outline that area, not the content block, while an item is over it', () => {
    expect(rule('Canvas.css', '.ui-designer-canvas-empty.over')).toContain(
      'box-shadow: inset 0 0 0 1px var(--palette-info-light);',
    );
  });

  it('should give the no-GUI state no drop target', () => {
    render(<NoGuiState />);

    expect(mocks.dropSpec).toBeNull();
    expect(mocks.dropTarget).toBeNull();
  });
});
