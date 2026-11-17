import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Loading, LoadingSpinner } from './Loading';
import type { SpinnerSize } from './types';

const spinnerPixels = () => {
  const svg = screen.getByRole('status', { name: 'Loading' }).querySelector('svg');
  return [svg?.getAttribute('width'), svg?.getAttribute('height')];
};

describe('Loading', () => {
  describe('when rendered with no props', () => {
    it('should show a 32 pixel spinner over a dimmer', () => {
      render(<Loading />);

      expect(spinnerPixels()).toEqual(['32', '32']);
      expect(screen.getByTestId('loading-dimmer')).toBeTruthy();
    });

    it('should keep the class name that the panels style', () => {
      const { container } = render(<Loading />);

      expect(container.firstElementChild?.className).toBe('loading');
      expect(screen.getByTestId('loading-dimmer').classList.contains('loading-dimmer')).toBe(true);
    });
  });

  describe('when the dimmer is off', () => {
    it('should show the spinner with no dimmer', () => {
      render(<Loading dimmer={false} />);

      expect(screen.getByRole('status', { name: 'Loading' })).toBeTruthy();
      expect(screen.queryByTestId('loading-dimmer')).toBeNull();
    });
  });

  describe('when a size is given', () => {
    it('should pass the size on to the spinner', () => {
      render(<Loading size="large" />);

      expect(spinnerPixels()).toEqual(['48', '48']);
    });
  });
});

describe('LoadingSpinner', () => {
  const sizes: [SpinnerSize, string][] = [
    ['mini', '14'],
    ['tiny', '16'],
    ['small', '24'],
    ['medium', '32'],
    ['large', '48'],
    ['big', '52'],
    ['huge', '58'],
    ['massive', '64'],
  ];

  it.each(sizes)('should draw the %s spinner at %s pixels', (size, pixels) => {
    render(<LoadingSpinner size={size} />);

    expect(spinnerPixels()).toEqual([pixels, pixels]);
  });

  it('should draw the medium spinner when no size is given', () => {
    render(<LoadingSpinner />);

    expect(spinnerPixels()).toEqual(['32', '32']);
  });
});
