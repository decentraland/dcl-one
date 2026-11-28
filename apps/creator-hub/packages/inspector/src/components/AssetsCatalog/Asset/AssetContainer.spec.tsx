import type { ReactNode } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Asset as AssetType } from '../../../lib/logic/catalog';
import AssetContainer from './AssetContainer';

vi.mock('../../ui', () => ({
  InfoTooltip: ({
    trigger,
    position,
    onOpen,
    onClose,
  }: {
    trigger: ReactNode;
    position: string;
    onOpen: () => void;
    onClose: () => void;
  }) => (
    <div
      data-testid="asset-tile"
      data-position={position}
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

const boxAt = (top: number, height = 80) => ({ top, bottom: top + height }) as DOMRect;

const hoverTileInList = (tileTop: number) => {
  render(
    <div className="assets-catalog">
      <div className="assets-catalog-header" />
      <AssetContainer
        value={{ ...smartItem('someone'), composite: undefined } as unknown as AssetType}
      />
    </div>,
  );
  const tile = screen.getByText('Door');
  const header = tile.closest('.assets-catalog')!.querySelector('.assets-catalog-header')!;
  vi.spyOn(header, 'getBoundingClientRect').mockReturnValue(boxAt(40, 60));
  vi.spyOn(tile, 'getBoundingClientRect').mockReturnValue(boxAt(tileTop));
  fireEvent.mouseEnter(screen.getByTestId('asset-tile'));
  return screen.getByTestId('asset-tile').dataset.position;
};

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

  describe('when a tile with room above it under the catalog header is hovered', () => {
    it('should show the name above the tile', () => {
      expect(hoverTileInList(160)).toBe('top center');
    });
  });

  describe('when a tile scrolled up against the catalog header is hovered', () => {
    it('should show the name below the tile, clear of the asset pack path', () => {
      expect(hoverTileInList(104)).toBe('bottom center');
    });

    describe('and the tile also sits at the bottom of the window', () => {
      beforeEach(() => {
        vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(200);
      });

      it('should show the name to the left of the tile', () => {
        expect(hoverTileInList(104)).toBe('left center');
      });
    });
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
