import { render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import Spinner from '@ui/atoms/HubSpinner';

import { TestProviders } from '../../tests/utils/TestProviders';
import { mountThemeStyles } from '../../tests/utils/themeStyles';

function ringOf(spinner: HTMLElement): CSSStyleDeclaration {
  const ring = spinner.querySelector('circle');
  if (!ring) throw new Error('The spinner has no ring');
  return getComputedStyle(ring);
}

describe('ui3 atoms in the renderer', () => {
  describe('when a spec renders the shared spinner', () => {
    it('should draw it with the React of this workspace', () => {
      render(
        <TestProviders>
          <Spinner size={20} />
        </TestProviders>,
      );

      const spinner = screen.getByRole('status', { name: 'Loading' });

      expect(spinner.style.getPropertyValue('--sz')).toBe('20px');
      expect(spinner.querySelector('svg')?.getAttribute('width')).toBe('20');
    });
  });

  describe('when the app stylesheets are on the page', () => {
    let unmountThemeStyles: () => void;

    beforeEach(() => {
      unmountThemeStyles = mountThemeStyles();
      render(
        <TestProviders>
          <Spinner
            ring="wide"
            label="Wide ring"
          />
          <Spinner label="Atom ring" />
        </TestProviders>,
      );
    });

    afterEach(() => {
      unmountThemeStyles();
    });

    it('should give a spinner with the wide ring the ring it had before the atom', () => {
      const ring = ringOf(screen.getByRole('status', { name: 'Wide ring' }));

      expect(ring.getPropertyValue('stroke-linecap')).toBe('butt');
      expect(ring.getPropertyValue('stroke-width')).toBe('4.091px');
      expect(ring.getPropertyValue('animation')).toContain('1.4s');
    });

    it('should leave a spinner without it as the atom draws it', () => {
      const ring = ringOf(screen.getByRole('status', { name: 'Atom ring' }));

      expect(ring.getPropertyValue('stroke-linecap')).toBe('');
      expect(ring.getPropertyValue('stroke-width')).toBe('');
      expect(ring.getPropertyValue('animation')).toBe('');
    });
  });
});
