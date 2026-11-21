import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { WIDGET_LIST } from '../shared/widget-catalog';
import { Palette } from './Palette';

vi.mock('react-dnd', () => ({ useDrag: () => [{ isDragging: false }, () => {}] }));
vi.mock('../../../redux/hooks', () => ({ useAppSelector: () => false }));
vi.mock('../code/store', () => ({ useCodeState: () => ({ roots: [{ name: 'MainUI' }] }) }));

const svgOf = (ui: React.ReactElement) => {
  const html = render(ui).container.querySelector('svg')!.outerHTML;
  cleanup();
  return html;
};

describe('the bottom-panel cards', () => {
  afterEach(cleanup);

  it('should draw each widget with its library icon, sized by the card', () => {
    const expected = WIDGET_LIST.map(w => svgOf(<w.Icon fontSize="inherit" />));
    const { container } = render(<Palette />);

    expect(
      [...container.querySelectorAll('.ui-designer-palette-icon svg')].map(s => s.outerHTML),
    ).toEqual(expected);
  });
});
