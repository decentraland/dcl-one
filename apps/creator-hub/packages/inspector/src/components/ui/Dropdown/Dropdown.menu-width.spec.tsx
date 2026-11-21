import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { Dropdown } from '.';

vi.mock('../../../hooks/useContainerSize', () => ({
  useContainerSize: () => ({ width: 56, height: 26 }),
}));

const options = [
  { value: 0, label: 'px' },
  { value: 1, label: 'Percentage' },
];

function openList() {
  render(
    <Dropdown
      aria-label="Unit"
      options={options}
      value={0}
    />,
  );
  fireEvent.click(screen.getByRole('combobox', { hidden: true }));
  return screen.getByRole('listbox', { hidden: true });
}

describe('when a field dropdown narrower than its options opens', () => {
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(72);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('should not pin the list to the trigger width', () => {
    expect(openList().style.width).toBe('');
  });

  it('should keep the list at least as wide as the whole trigger', () => {
    expect(openList().style.minWidth).toBe('72px');
  });

  it('should render every option label', () => {
    const list = openList();
    expect(list.textContent).toContain('px');
    expect(list.textContent).toContain('Percentage');
  });
});
