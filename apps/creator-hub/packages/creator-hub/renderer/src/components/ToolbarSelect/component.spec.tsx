import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SelectOption } from '@ui/atoms/Select';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { ToolbarSelect } from '.';

describe('ToolbarSelect', () => {
  afterEach(cleanup);

  describe('when the caller passes its own menu class', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <ToolbarSelect
            value="a"
            menuClassName="CallerMenu"
          >
            <SelectOption value="a">A</SelectOption>
          </ToolbarSelect>
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('combobox'));
    });

    it('should keep both the toolbar and the caller menu classes', () => {
      const menu = screen.getByRole('listbox');

      expect(menu.classList.contains('SelectMenu')).toBe(true);
      expect(menu.classList.contains('CallerMenu')).toBe(true);
    });

    it('should still leave the page scrollbar alone', () => {
      expect(document.body.style.overflow).not.toBe('hidden');
    });
  });

  describe('when an option is picked', () => {
    let onChange: ReturnType<typeof vi.fn<(value: string) => void>>;

    beforeEach(() => {
      onChange = vi.fn<(value: string) => void>();
      render(
        <TestProviders>
          <span id="sort-label">Sort by</span>
          <ToolbarSelect
            aria-labelledby="sort-label"
            value="a"
            onChange={onChange}
          >
            <SelectOption value="a">Newest</SelectOption>
            <SelectOption
              value="b"
              className="sort-item"
            >
              Name
            </SelectOption>
            <SelectOption
              value="c"
              disabled
            >
              Size
            </SelectOption>
          </ToolbarSelect>
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('combobox', { name: 'Sort by' }));
    });

    it('should show the text of the selected option on the trigger', () => {
      expect(screen.getByRole('combobox', { name: 'Sort by' }).textContent).toBe('Newest');
    });

    it('should keep the class of each option', () => {
      expect(screen.getByRole('option', { name: 'Name' }).classList.contains('sort-item')).toBe(
        true,
      );
    });

    it('should give the new value', () => {
      fireEvent.click(screen.getByRole('option', { name: 'Name' }));

      expect(onChange).toHaveBeenCalledExactlyOnceWith('b');
    });

    it('should not report a disabled option', () => {
      fireEvent.click(screen.getByRole('option', { name: 'Size' }));

      expect(onChange).not.toHaveBeenCalled();
    });
  });
});
