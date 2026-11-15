import type { ReactNode } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Asset as AssetType } from '../../../lib/logic/catalog';
import AssetContainer from './AssetContainer';

vi.mock('../../ui', () => ({
  InfoTooltip: ({
    trigger,
    onOpen,
    onClose,
  }: {
    trigger: ReactNode;
    onOpen: () => void;
    onClose: () => void;
  }) => (
    <div
      data-testid="asset-tile"
      onMouseEnter={onOpen}
      onMouseLeave={onClose}
    >
      {trigger}
    </div>
  ),
}));

vi.mock('./Asset', async () => {
  const { forwardRef } = await import('react');
  return {
    default: forwardRef<HTMLDivElement, { value: { name: string } }>(({ value }, ref) => (
      <div ref={ref}>{value.name}</div>
    )),
  };
});

const smartItem = (author: string): AssetType =>
  ({
    id: 'door',
    name: 'Door',
    category: 'utils',
    tags: [],
    contents: {},
    description: 'Opens on click.',
    author,
    composite: {
      version: 1,
      components: [
        { name: 'a', data: {} },
        { name: 'b', data: {} },
      ],
    },
  }) as unknown as AssetType;

const hoverPastTheDelay = () => {
  fireEvent.mouseEnter(screen.getByTestId('asset-tile'));
  act(() => {
    vi.advanceTimersByTime(500);
  });
};

describe('AssetContainer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe('when a smart item made by the Decentraland Foundation is hovered', () => {
    it('should show the Decentraland logomark at 16 pixels next to the author', () => {
      render(<AssetContainer value={smartItem('Decentraland Foundation')} />);

      hoverPastTheDelay();

      const author = screen.getByText('Created by').parentElement;
      const logomark = author?.querySelector('svg');
      expect(author?.textContent).toContain('Decentraland Foundation');
      expect(logomark?.getAttribute('viewBox')).toBe('0 0 54 54');
      expect(logomark?.getAttribute('width')).toBe('16');
      expect(logomark?.getAttribute('height')).toBe('16');
      expect(logomark?.getAttribute('fill')).toBe('currentColor');
    });

    it('should show the item details in a tooltip', () => {
      render(<AssetContainer value={smartItem('Decentraland Foundation')} />);

      hoverPastTheDelay();

      const tooltip = screen.getByRole('tooltip');
      expect(tooltip.textContent).toContain('Opens on click.');
      expect(tooltip.classList.contains('SmartItemTooltip')).toBe(true);
    });

    it('should not show the item details before the delay ends', () => {
      render(<AssetContainer value={smartItem('Decentraland Foundation')} />);

      fireEvent.mouseEnter(screen.getByTestId('asset-tile'));
      act(() => {
        vi.advanceTimersByTime(499);
      });

      expect(screen.queryByText('Created by')).toBeNull();
    });
  });

  describe('when a smart item made by someone else is hovered', () => {
    it('should show the author with no logomark', async () => {
      vi.spyOn(globalThis, 'fetch').mockResolvedValue(
        new Response(JSON.stringify([]), { status: 200 }),
      );
      render(<AssetContainer value={smartItem('0xabc')} />);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(0);
      });

      hoverPastTheDelay();

      const author = screen.getByText('Created by').parentElement;
      expect(author?.textContent).toContain('0xabc');
      expect(author?.querySelector('svg')).toBeNull();
    });
  });
});
