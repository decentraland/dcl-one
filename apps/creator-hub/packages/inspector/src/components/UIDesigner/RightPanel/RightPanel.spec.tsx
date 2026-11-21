import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import type * as StoreNodes from '../code/store-nodes';
import { RightPanel } from './RightPanel';

type Node = { entity: number; type: string; children: Node[] };

const mocks = vi.hoisted(() => ({
  selectedNodes: [] as number[],
  mobileHudSelected: false,
  root: null as Node | null,
  filename: null as string | null,
}));

vi.mock('../../../redux/hooks', () => ({
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({
      uiDesigner: {
        selectedNodes: mocks.selectedNodes,
        mobileHudSelected: mocks.mobileHudSelected,
      },
    }),
}));

vi.mock('../code/store', async () => {
  const nodes = await vi.importActual<typeof StoreNodes>('../code/store-nodes');
  return {
    findCodeNode: nodes.findCodeNode,
    useCodeState: () => ({
      parsed: mocks.root ? { root: mocks.root } : null,
      filename: mocks.filename,
    }),
  };
});

vi.mock('./PropertyPanel', () => ({ PropertyPanel: () => <div data-testid="properties" /> }));
vi.mock('./MobileHudPanel', () => ({ MobileHudPanel: () => <div data-testid="mobile-hud" /> }));
vi.mock('./LogicPanel/CodeVariablesPanel', () => ({
  CodeVariablesPanel: () => <div data-testid="logic" />,
}));
vi.mock('./LogicPanel/CodePropsPanel', () => ({ CodePropsPanel: () => null }));
vi.mock('./LogicPanel/CodeCallbacksPanel', () => ({ CodeCallbacksPanel: () => null }));
vi.mock('./LogicPanel/ComponentRefPanel', () => ({ ComponentRefPanel: () => null }));

const node = (entity: number, children: Node[] = []): Node => ({
  entity,
  type: 'UiEntity',
  children,
});

const rail = () => document.querySelector<HTMLElement>('.ui-designer-right-rail')!;
const tablist = () => rail().querySelector('[role="tablist"]');
const tab = (name: 'Properties' | 'Logic') =>
  [...rail().querySelectorAll<HTMLElement>('[role="tab"]')].find(t => t.textContent === name)!;

beforeEach(() => {
  mocks.selectedNodes = [];
  mocks.mobileHudSelected = false;
  mocks.root = node(1, [node(2)]);
  mocks.filename = 'src/ui/GUI.tsx';
});

afterEach(cleanup);

describe('when no node is selected', () => {
  it.each([
    ['there is no GUI', () => ((mocks.root = null), (mocks.filename = null))],
    ['the GUI is empty', () => (mocks.root = null)],
    ['the GUI has nodes but none is selected', () => undefined],
  ])('should show the tab-less Empty state when %s', (_, arrange) => {
    arrange();
    render(<RightPanel />);

    expect(tablist()).toBeNull();
    expect(rail().querySelector('svg')).toBeNull();
    const empty = rail().querySelector('.ui-designer-empty-state--panel')!;
    expect(empty.querySelector('.ui-designer-empty-state-title')!.textContent).toBe(
      'No node selected',
    );
    expect(empty.querySelector('.ui-designer-empty-state-message')!.textContent).toBe(
      'Select a node to start editing its properties and logic.',
    );
    expect(screen.queryByTestId('properties')).toBeNull();
  });

  it('should set the word node apart from the rest of the body', () => {
    render(<RightPanel />);

    expect(rail().querySelector('.ui-designer-empty-state-message strong')).toBeNull();
    expect(rail().querySelector('.ui-designer-empty-state-emphasis')!.textContent!.trim()).toBe(
      'node',
    );
  });

  it('should treat a selection that resolves to no node as none', () => {
    mocks.selectedNodes = [99];
    render(<RightPanel />);

    expect(tablist()).toBeNull();
  });
});

describe('when a node is selected again after the panel was empty', () => {
  it('should bring the tabs back on the tab last used', () => {
    mocks.selectedNodes = [2];
    const { rerender } = render(<RightPanel />);
    fireEvent.click(tab('Logic'));

    mocks.selectedNodes = [];
    rerender(<RightPanel />);
    expect(tablist()).toBeNull();

    mocks.selectedNodes = [1];
    rerender(<RightPanel />);
    expect(tab('Logic').getAttribute('aria-selected')).toBe('true');
    expect(screen.getByTestId('logic')).toBeTruthy();
  });
});

describe('when the MobileHUD is selected', () => {
  it('should show the MobileHUD panel, not the Empty state', () => {
    mocks.mobileHudSelected = true;
    render(<RightPanel />);

    expect(screen.getByTestId('mobile-hud')).toBeTruthy();
    expect(rail().querySelector('.ui-designer-empty-state')).toBeNull();
  });
});
