import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { FormTextInput } from '.';

describe('FormTextInput', () => {
  afterEach(cleanup);

  describe('when it gets attributes for the control', () => {
    it('should name the text box with them', () => {
      render(
        <TestProviders>
          <FormTextInput inputProps={{ 'aria-label': 'Name' }} />
        </TestProviders>,
      );

      expect(screen.getByRole('textbox', { name: 'Name' })).toBeTruthy();
    });
  });

  describe('when the user types', () => {
    it('should call onChange with the text', () => {
      const onChange = vi.fn();
      render(
        <TestProviders>
          <FormTextInput
            inputProps={{ 'aria-label': 'Name' }}
            value=""
            onChange={event => onChange(event.target.value)}
          />
        </TestProviders>,
      );

      fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), {
        target: { value: 'Plaza' },
      });

      expect(onChange).toHaveBeenCalledWith('Plaza');
    });
  });

  describe('when it has an error', () => {
    it('should mark the text box as invalid', () => {
      render(
        <TestProviders>
          <FormTextInput
            inputProps={{ 'aria-label': 'Name' }}
            error
          />
        </TestProviders>,
      );

      expect(screen.getByRole('textbox', { name: 'Name' }).getAttribute('aria-invalid')).toBe(
        'true',
      );
    });
  });

  describe('when it is multiline', () => {
    it('should be a text area', () => {
      render(
        <TestProviders>
          <FormTextInput
            inputProps={{ 'aria-label': 'Description' }}
            multiline
          />
        </TestProviders>,
      );

      expect(screen.getByRole('textbox', { name: 'Description' }).tagName).toBe('TEXTAREA');
    });
  });
});
