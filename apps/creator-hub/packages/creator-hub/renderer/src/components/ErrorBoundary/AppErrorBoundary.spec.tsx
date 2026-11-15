import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { captureException } from '@sentry/electron/renderer';

import { mountThemeStyles } from '../../../tests/utils/themeStyles';

import { AppErrorBoundary } from '.';

vi.mock('@sentry/electron/renderer', () => ({ captureException: vi.fn() }));

const crash = { enabled: true };

function Crashing() {
  if (crash.enabled) {
    throw new Error('provider failed');
  }
  return <p>Recovered app</p>;
}

// Rendered with no store, router, theme or translation provider, and no locale loaded:
// the app-level boundary sits outside all of them.
describe('AppErrorBoundary', () => {
  let unmountThemeStyles: () => void;

  beforeEach(() => {
    unmountThemeStyles = mountThemeStyles();
    crash.enabled = true;
    vi.mocked(captureException).mockClear();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    render(
      <AppErrorBoundary name="app">
        <Crashing />
      </AppErrorBoundary>,
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    unmountThemeStyles();
  });

  describe('when something outside every provider throws', () => {
    it('should show the fallback in English', () => {
      expect(screen.getByTestId('app-error-fallback')).toBeTruthy();
      expect(screen.getByText('Something went wrong')).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Reload' })).toBeTruthy();
    });

    it('should style the fallback with the dark theme', () => {
      // No provider gives the colour: the theme stylesheets of the app set the token on the page root.
      expect(getComputedStyle(screen.getByText(/reload the window/)).color).toBe(
        'rgba(240, 240, 240, 0.7)',
      );
    });

    it('should report the error to Sentry with the boundary tags', () => {
      expect(captureException).toHaveBeenCalledTimes(1);
      expect(captureException).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'provider failed' }),
        expect.objectContaining({ tags: { source: 'error-boundary', boundary: 'app' } }),
      );
    });

    describe('and the user clicks Try again once the cause is gone', () => {
      beforeEach(() => {
        crash.enabled = false;
        fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
      });

      it('should render the tree again', () => {
        expect(screen.getByText('Recovered app')).toBeTruthy();
        expect(screen.queryByTestId('app-error-fallback')).toBeNull();
      });
    });
  });
});
