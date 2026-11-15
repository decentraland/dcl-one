import type { ReactNode } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { Select } from '.';

// The callers give elements that have a `value` prop. The Select reads the props and draws the option.
function Item(_props: {
  value: string;
  disabled?: boolean;
  className?: string;
  children?: ReactNode;
}) {
  return null;
}

describe('Select', () => {
  afterEach(cleanup);

  describe('when the caller passes its own MenuProps', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Select
            value="a"
            MenuProps={{ className: 'CallerMenu' }}
          >
            <Item value="a">A</Item>
          </Select>
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('combobox'));
    });

    it('should keep both the shared and the caller menu classes', () => {
      const menu = screen.getByRole('listbox');

      expect(menu.classList.contains('SelectMenu')).toBe(true);
      expect(menu.classList.contains('CallerMenu')).toBe(true);
    });

    it('should still leave the page scrollbar alone', () => {
      expect(document.body.style.overflow).not.toBe('hidden');
    });
  });

  describe('when the options are elements with a value', () => {
    let onChange: ReturnType<typeof vi.fn<(event: Event) => void>>;

    beforeEach(() => {
      onChange = vi.fn<(event: Event) => void>();
      render(
        <TestProviders>
          <span id="sort-label">Sort by</span>
          <Select
            labelId="sort-label"
            name="sort"
            value="a"
            onChange={onChange}
          >
            <Item value="a">Newest</Item>
            <Item
              value="b"
              className="sort-item"
            >
              Name
            </Item>
            <Item
              value="c"
              disabled
            >
              Size
            </Item>
          </Select>
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('combobox', { name: 'Sort by' }));
    });

    it('should show the text of the selected option on the trigger', () => {
      expect(screen.getByRole('combobox', { name: 'Sort by' }).textContent).toBe('Newest');
    });

    it('should draw one option for each element and keep its class', () => {
      expect(screen.getAllByRole('option').map(option => option.textContent)).toEqual([
        'Newest',
        'Name',
        'Size',
      ]);
      expect(screen.getByRole('option', { name: 'Name' }).classList.contains('sort-item')).toBe(
        true,
      );
    });

    it('should give the new value and the field name in the target of the event', () => {
      fireEvent.click(screen.getByRole('option', { name: 'Name' }));

      expect(onChange).toHaveBeenCalledTimes(1);
      expect(onChange.mock.calls[0][0].target).toEqual({ value: 'b', name: 'sort' });
    });

    it('should not report a disabled option', () => {
      fireEvent.click(screen.getByRole('option', { name: 'Size' }));

      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
