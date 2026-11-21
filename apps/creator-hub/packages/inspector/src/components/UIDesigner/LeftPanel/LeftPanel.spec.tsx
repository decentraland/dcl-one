import { readFileSync } from 'fs';
import { join } from 'path';
import { afterEach, describe, expect, it, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { UINode } from '../shared/tree-model';
import type * as AnalyticsModule from '../../../lib/logic/analytics';
import { Event } from '../../../lib/logic/analytics';
import { LeftPanel } from './LeftPanel';

const mocks = vi.hoisted(() => ({
  createRoot: vi.fn(),
  selectedNodes: [1] as number[],
  mobileHudSelected: false,
  picker: null as null | { parent?: unknown; offset?: number; onDismiss?: () => void },
  track: vi.fn(),
  roots: [] as { name: string; filename: string; topLevel: boolean }[],
  filename: null as string | null,
  tree: null as UINode | null,
  emptyRoot: false,
}));

vi.mock('../../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

vi.mock('../code/store', () => ({
  createRoot: mocks.createRoot,
  useCodeState: () => ({
    roots: mocks.roots,
    filename: mocks.filename,
    emptyRoot: mocks.emptyRoot,
  }),
}));

vi.mock('../shared/useUINodeTree', () => ({ useUINodeTree: () => mocks.tree }));

vi.mock('../../../redux/hooks', () => ({
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({
      uiDesigner: {
        selectedNodes: mocks.selectedNodes,
        mobileHudSelected: mocks.mobileHudSelected,
      },
    }),
  useAppDispatch: () => vi.fn(),
}));

vi.mock('./NodeTree', () => ({
  NodeTree: ({ filter }: { filter?: string }) => <div data-testid="nodes">{filter}</div>,
}));
vi.mock('./CodeRootsList', () => ({
  CodeRootsList: ({ filter }: { filter?: string }) => <div data-testid="guis">{filter}</div>,
}));
vi.mock('./WidgetPicker', () => ({
  WidgetPicker: (props: { parent?: unknown; offset?: number; onDismiss?: () => void }) => {
    mocks.picker = props;
    return null;
  },
}));

const root = (over: Partial<UINode> = {}): UINode =>
  ({ entity: 1, type: 'UiEntity', name: 'UiEntity', children: [], ...over }) as UINode;

const search = () => screen.getByLabelText('Search GUIs and nodes');
const guis = () => screen.queryByTestId('guis');
const nodes = () => screen.queryByTestId('nodes');

function type(term: string) {
  fireEvent.change(search(), { target: { value: term } });
  act(() => {
    vi.advanceTimersByTime(1);
  });
}

function renderRail(term?: string) {
  render(<LeftPanel />);
  if (term !== undefined) type(term);
}

beforeEach(() => {
  vi.useFakeTimers();
  mocks.createRoot.mockClear();
  mocks.track.mockClear();
  mocks.roots = [{ name: 'MainUI', filename: 'src/ui/MainUI.tsx', topLevel: true }];
  mocks.filename = 'src/ui/MainUI.tsx';
  mocks.tree = root({ uiName: 'Sidebar', children: [] } as Partial<UINode>);
  mocks.emptyRoot = false;
  mocks.selectedNodes = [1];
  mocks.mobileHudSelected = false;
  mocks.picker = null;
});

const header = (name: 'New GUI' | 'Add Node') => {
  const el = screen.getByLabelText(name);
  expect(el.tagName).toBe('BUTTON');
  return el;
};

afterEach(() => {
  vi.useRealTimers();
});

describe('when no GUI is selected', () => {
  it('should not show the Nodes section at all', () => {
    mocks.filename = null;
    mocks.tree = null;
    renderRail();

    expect(guis()).not.toBeNull();
    expect(nodes()).toBeNull();
    expect(screen.queryByText('Nodes')).toBeNull();
  });
});

describe('when the MobileHUD is selected', () => {
  it('should not show the Nodes section', () => {
    mocks.mobileHudSelected = true;
    renderRail();

    expect(nodes()).toBeNull();
    expect(screen.queryByLabelText('Add Node')).toBeNull();
  });
});

describe('when a GUI is selected', () => {
  it('should show both sections', () => {
    renderRail();

    expect(guis()).not.toBeNull();
    expect(nodes()).not.toBeNull();
  });

  it('should split the rail without a resize handle', () => {
    const { container } = render(<LeftPanel />);

    expect(container.querySelector('[data-panel-resize-handle-id]')).toBeNull();
    expect(container.querySelector('.ui-designer-rail-handle')).toBeNull();
  });
});

describe('the section headers', () => {
  it('should each be one button, named New GUI and Add Node, holding its label', () => {
    renderRail();

    expect(header('New GUI').textContent).toContain('GUIs');
    expect(header('Add Node').textContent).toContain('Nodes');
    expect(screen.queryByLabelText('Add widget')).toBeNull();
  });

  it('should create a GUI and track it from the GUIs row', () => {
    renderRail();
    fireEvent.click(header('New GUI'));

    expect(mocks.createRoot).toHaveBeenCalled();
    expect(mocks.track).toHaveBeenCalledWith(Event.CREATE_UI, {});
  });

  it('should leave no ripple after either row is pressed', () => {
    const { container } = render(<LeftPanel />);
    fireEvent.click(header('New GUI'));
    fireEvent.click(header('Add Node'));

    expect(container.querySelectorAll('.ui-designer-rail-add-ripple')).toHaveLength(0);
  });
});

describe('when adding a node', () => {
  it('should add under the selected node', () => {
    mocks.selectedNodes = [7];
    renderRail();
    fireEvent.click(header('Add Node'));

    expect(mocks.picker?.parent).toBe(7);
  });

  it('should place the menu 8px below the NODES chip', () => {
    renderRail();
    fireEvent.click(header('Add Node'));

    expect(mocks.picker?.offset).toBe(8);
  });

  describe('and nothing is selected', () => {
    beforeEach(() => {
      mocks.selectedNodes = [];
    });

    it('should stay enabled', () => {
      renderRail();

      expect((header('Add Node') as HTMLButtonElement).disabled).toBe(false);
    });

    it("should append to the GUI's top-level element, never replacing the GUI's JSX", () => {
      renderRail();
      fireEvent.click(header('Add Node'));

      expect(mocks.picker?.parent).toBe(1);
    });
  });

  describe('and the GUI is empty', () => {
    it('should set the first element as the root', () => {
      mocks.selectedNodes = [];
      mocks.tree = null;
      mocks.emptyRoot = true;
      renderRail();
      fireEvent.click(header('Add Node'));

      expect(mocks.picker?.parent).toBeNull();
    });
  });
});

describe('when searching', () => {
  it('should pass the same term to both lists', () => {
    renderRail('side');

    expect(guis()!.textContent).toBe('side');
    expect(nodes()!.textContent).toBe('side');
  });

  it('should keep both headers and lists when nothing matches, with both rows working', () => {
    renderRail('nothinghere');

    expect(guis()).not.toBeNull();
    expect(nodes()).not.toBeNull();
    fireEvent.click(header('New GUI'));
    fireEvent.click(header('Add Node'));
    expect(mocks.createRoot).toHaveBeenCalled();
    expect(mocks.picker).not.toBeNull();
  });

  it('should clear the term from a named Clear search button', () => {
    renderRail('nothinghere');
    const clear = screen.getByLabelText('Clear search');
    expect(clear.tagName).toBe('BUTTON');
    fireEvent.click(clear);
    act(() => {
      vi.advanceTimersByTime(1);
    });

    expect(guis()!.textContent).toBe('');
    expect(nodes()!.textContent).toBe('');
  });
});

describe('when the Add Node menu opens from the NODES row', () => {
  it('should announce the row as a dialog opener, expanded only while the menu is open', () => {
    renderRail();
    const row = header('Add Node');

    expect(row.getAttribute('aria-haspopup')).toBe('dialog');
    expect(row.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(row);
    expect(row.getAttribute('aria-expanded')).toBe('true');
    act(() => mocks.picker?.onDismiss?.());
    expect(row.getAttribute('aria-expanded')).toBe('false');
  });

  it('should leave New GUI without a popup, since it opens nothing', () => {
    renderRail();

    expect(header('New GUI').hasAttribute('aria-haspopup')).toBe(false);
    expect(header('New GUI').hasAttribute('aria-expanded')).toBe(false);
  });

  it('should keep the row and its chip in their On look from aria-expanded, not the pointer', () => {
    const stylesheet = readFileSync(join(__dirname, '..', 'UIDesigner.css'), 'utf8');
    const rules = new Map(
      Array.from(stylesheet.matchAll(/([^{}]+)\{([^}]*)\}/g), ([, selector, body]) => [
        selector.replace(/\s+/g, ' ').trim(),
        body.replace(/\s+/g, ' ').trim(),
      ]),
    );

    expect(rules.get('.ui-designer-rail-header[aria-expanded="true"]')).toContain(
      'background-color: var(--palette-whiteTransparent-faint);',
    );
    expect(
      rules.get('.ui-designer-rail-header[aria-expanded="true"] .ui-designer-rail-add'),
    ).toContain('background-color: var(--palette-whiteTransparent-subtle);');
  });
});
