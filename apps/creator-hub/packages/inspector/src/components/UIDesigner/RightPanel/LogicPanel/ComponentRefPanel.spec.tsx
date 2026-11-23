import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { Prefab } from '../../../Icons/figma';
import type { CodeUINode } from '../../code/types';
import { ComponentRefPanel } from './ComponentRefPanel';

vi.mock('../../code/store', () => ({
  selectRootFile: vi.fn(),
  spliceInstanceProp: vi.fn(),
  unsetInstanceProp: vi.fn(),
  useCodeState: () => ({ componentTrees: {}, roots: [] }),
}));

const instance = {
  entity: 5,
  type: 'UiEntity',
  name: 'Card',
  componentRef: { name: 'Card', props: [] },
  children: [],
} as unknown as CodeUINode;

describe('ComponentRefPanel header', () => {
  afterEach(cleanup);

  it('should draw the nested GUI with the library Prefab glyph, sized by the header', () => {
    const expected = render(<Prefab fontSize="inherit" />).container.querySelector(
      'svg',
    )!.outerHTML;
    cleanup();
    const { container } = render(<ComponentRefPanel node={instance} />);

    expect(container.querySelector('.ui-designer-code-gui-header svg')!.outerHTML).toBe(expected);
  });
});
