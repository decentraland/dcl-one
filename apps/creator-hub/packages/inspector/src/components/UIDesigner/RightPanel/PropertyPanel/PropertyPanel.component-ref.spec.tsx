import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { initialState } from '../../../../redux/ui-designer';
import { Prefab } from '../../../Icons/figma';
import type { CodeUINode } from '../../code/types';
import { PropertyPanel } from './PropertyPanel';

const instance = {
  entity: 5,
  type: 'UiEntity',
  name: 'Card',
  componentRef: { name: 'Card', props: [] },
  children: [],
} as unknown as CodeUINode;

vi.mock('../../../../redux/hooks', () => ({
  useAppDispatch: () => vi.fn(),
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({ uiDesigner: { ...initialState, selectedNodes: [5] } }),
}));

vi.mock('../../code/store', async importOriginal => ({
  ...(await importOriginal<object>()),
  useCodeState: () => ({
    parsed: {
      root: { entity: 1, type: 'UiEntity', name: 'Root', children: [instance] },
    },
    roots: [],
    filename: null,
    componentTrees: {},
  }),
}));

describe('when a nested GUI is selected', () => {
  afterEach(cleanup);

  it('should head the panel with the library Prefab glyph at 18', () => {
    const expected = render(
      <Prefab
        fontSize="inherit"
        className="ui-designer-component-ref-glyph"
      />,
    ).container.querySelector('svg')!.outerHTML;
    cleanup();
    const { container } = render(<PropertyPanel />);

    expect(container.querySelector('.ui-designer-empty-state-icon svg')!.outerHTML).toBe(expected);
  });
});
