import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../../tests/utils/TestProviders';

import { SelectCheckbox } from '.';

const renderBox = (variant: 'primary' | 'secondary') =>
  render(
    <TestProviders>
      <SelectCheckbox
        variant={variant}
        indeterminate
        inputProps={{ 'aria-label': 'Select all' }}
      />
    </TestProviders>,
  );

const TOKENS = { '--panel': '#161518', '--brand': '#FF2D55' };

describe('SelectCheckbox', () => {
  beforeEach(() => {
    for (const [token, value] of Object.entries(TOKENS))
      document.documentElement.style.setProperty(token, value);
  });

  afterEach(() => {
    cleanup();
    for (const token of Object.keys(TOKENS)) document.documentElement.style.removeProperty(token);
  });

  describe('when the primary box is mixed', () => {
    it('should draw a filled Primary box with a dark 10 × 2 dash', () => {
      renderBox('primary');
      const root = screen.getByRole('checkbox', { name: 'Select all' }).parentElement!;
      const dash = root.querySelector('[data-dash]') as HTMLElement;

      expect(dash).toBeTruthy();
      expect(getComputedStyle(dash).backgroundColor).toBe('#161518');
      expect(getComputedStyle(dash).width).toBe('10px');
      expect(getComputedStyle(dash.parentElement!).backgroundColor).toBe('#FF2D55');
    });
  });

  describe('when the caller passes attributes of a native checkbox', () => {
    it('should set them on the input', () => {
      const onFocus = vi.fn();
      const onBlur = vi.fn();
      render(
        <TestProviders>
          <SelectCheckbox
            id="terms"
            name="terms"
            value="yes"
            required
            tabIndex={-1}
            aria-label="Accept the terms"
            aria-describedby="terms-hint"
            onFocus={onFocus}
            onBlur={onBlur}
          />
        </TestProviders>,
      );
      const box = screen.getByRole('checkbox', { name: 'Accept the terms' }) as HTMLInputElement;

      fireEvent.focus(box);
      fireEvent.blur(box);

      expect(box.id).toBe('terms');
      expect(box.name).toBe('terms');
      expect(box.value).toBe('yes');
      expect(box.required).toBe(true);
      expect(box.tabIndex).toBe(-1);
      expect(box.getAttribute('aria-describedby')).toBe('terms-hint');
      expect(onFocus).toHaveBeenCalledTimes(1);
      expect(onBlur).toHaveBeenCalledTimes(1);
    });

    describe('and inputProps names the same attribute', () => {
      it('should let inputProps win', () => {
        render(
          <TestProviders>
            <SelectCheckbox
              aria-label="Row"
              inputProps={{ 'aria-label': 'Select all' }}
            />
          </TestProviders>,
        );

        expect(screen.getByRole('checkbox', { name: 'Select all' })).toBeTruthy();
      });
    });
  });
});
