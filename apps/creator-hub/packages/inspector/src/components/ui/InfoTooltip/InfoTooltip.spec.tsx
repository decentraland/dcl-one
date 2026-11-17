import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { InfoTooltip } from './index';

const CLOSE_DELAY = 120;

const pointerEnters = (element: Element) => fireEvent(element, new MouseEvent('mouseenter'));
const pointerLeaves = (element: Element) => fireEvent(element, new MouseEvent('mouseleave'));
const wait = (ms: number) =>
  act(() => {
    vi.advanceTimersByTime(ms);
  });

const rect = (left: number, top: number, width: number, height: number) =>
  ({
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    x: left,
    y: top,
    toJSON: () => ({}),
  }) as DOMRect;

describe('InfoTooltip', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe('when the pointer is not on the trigger', () => {
    it('should show the trigger and no tooltip', () => {
      render(
        <InfoTooltip
          text="Sets the size."
          trigger={<button>Size</button>}
        />,
      );

      expect(screen.getByRole('button', { name: 'Size' })).toBeTruthy();
      expect(screen.queryByRole('tooltip')).toBeNull();
    });

    it('should show an icon when no trigger is given', () => {
      const { container } = render(<InfoTooltip text="Sets the size." />);

      expect(container.querySelector('svg')).toBeTruthy();
      expect(container.firstElementChild?.className).toBe('InfoTooltipTrigger');
    });
  });

  describe('when the pointer enters the trigger', () => {
    it('should show the text and tell the caller', () => {
      const onOpen = vi.fn();
      render(
        <InfoTooltip
          text="Sets the size."
          trigger={<button>Size</button>}
          onOpen={onOpen}
        />,
      );

      pointerEnters(screen.getByRole('button', { name: 'Size' }).parentElement!);

      expect(screen.getByRole('tooltip').textContent).toContain('Sets the size.');
      expect(onOpen).toHaveBeenCalledTimes(1);
    });

    it('should mark the trigger as hovered', () => {
      const { container } = render(<InfoTooltip text="Sets the size." />);

      pointerEnters(container.firstElementChild!);

      expect(container.firstElementChild?.classList.contains('hovered')).toBe(true);
    });

    it('should keep the class names that the stylesheets use on the tooltip', () => {
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          className="custom-tooltip"
        />,
      );

      pointerEnters(container.firstElementChild!);

      const tooltip = screen.getByRole('tooltip');
      expect(tooltip.classList.contains('InfoTooltip')).toBe(true);
      expect(tooltip.classList.contains('custom-tooltip')).toBe(true);
    });

    it('should name the tooltip as the description of the trigger', () => {
      const { container } = render(<InfoTooltip text="Sets the size." />);

      pointerEnters(container.firstElementChild!);

      expect(container.firstElementChild?.getAttribute('aria-describedby')).toBe(
        screen.getByRole('tooltip').id,
      );
    });
  });

  describe('when a link is given', () => {
    it('should show a link that opens in a new tab', () => {
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          link="https://example.test/docs"
        />,
      );

      pointerEnters(container.firstElementChild!);

      const link = screen.getByRole('link', { name: 'Learn more' });
      expect(link.getAttribute('href')).toBe('https://example.test/docs');
      expect(link.getAttribute('target')).toBe('_blank');
    });

    it('should stay open while the pointer moves from the trigger to the link', () => {
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          link="https://example.test/docs"
        />,
      );
      pointerEnters(container.firstElementChild!);

      pointerLeaves(container.firstElementChild!);
      wait(CLOSE_DELAY - 1);
      fireEvent.mouseEnter(screen.getByRole('tooltip'));
      wait(CLOSE_DELAY * 10);

      expect(screen.getByRole('link', { name: 'Learn more' })).toBeTruthy();
    });
  });

  describe('when the pointer leaves the trigger', () => {
    it('should close after a short delay and tell the caller', () => {
      const onClose = vi.fn();
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          onClose={onClose}
        />,
      );
      pointerEnters(container.firstElementChild!);

      pointerLeaves(container.firstElementChild!);
      wait(CLOSE_DELAY);

      expect(screen.queryByRole('tooltip')).toBeNull();
      expect(container.firstElementChild?.classList.contains('hovered')).toBe(false);
      expect(onClose).toHaveBeenCalledTimes(1);
    });

    it('should close at once when the tooltip is not hoverable', () => {
      const { container } = render(
        <InfoTooltip
          text="Door"
          hoverable={false}
        />,
      );
      pointerEnters(container.firstElementChild!);

      pointerLeaves(container.firstElementChild!);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });

  describe('when the page scrolls', () => {
    it('should close the tooltip', () => {
      const { container } = render(<InfoTooltip text="Sets the size." />);
      pointerEnters(container.firstElementChild!);

      fireEvent.scroll(window);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });

    it('should keep the tooltip when hide on scroll is off', () => {
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          hideOnScroll={false}
        />,
      );
      pointerEnters(container.firstElementChild!);

      fireEvent.scroll(window);

      expect(screen.getByRole('tooltip')).toBeTruthy();
    });
  });

  describe('when the tooltip is disabled', () => {
    it('should not open', () => {
      const { container } = render(
        <InfoTooltip
          text="Reload scripts"
          disabled
        />,
      );

      pointerEnters(container.firstElementChild!);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });

  describe('when open on trigger mouse enter is off', () => {
    it('should not open', () => {
      const { container } = render(
        <InfoTooltip
          text="View Shortcuts"
          openOnTriggerMouseEnter={false}
        />,
      );

      pointerEnters(container.firstElementChild!);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });

  describe('when close on trigger click is on', () => {
    it('should close on a click on the trigger', () => {
      render(
        <InfoTooltip
          text="Refresh assets"
          trigger={<button>Refresh</button>}
          closeOnTriggerClick
        />,
      );
      const button = screen.getByRole('button', { name: 'Refresh' });
      pointerEnters(button.parentElement!);

      fireEvent.click(button);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });

    it('should open again when the pointer comes back', () => {
      render(
        <InfoTooltip
          text="Refresh assets"
          trigger={<button>Refresh</button>}
          closeOnTriggerClick
        />,
      );
      const button = screen.getByRole('button', { name: 'Refresh' });
      pointerEnters(button.parentElement!);
      fireEvent.click(button);
      pointerLeaves(button.parentElement!);
      wait(CLOSE_DELAY);

      pointerEnters(button.parentElement!);

      expect(screen.getByRole('tooltip').textContent).toContain('Refresh assets');
    });
  });

  describe('when the caller controls the open state', () => {
    it('should show the tooltip with no pointer on the trigger', () => {
      render(
        <InfoTooltip
          text="Door"
          open
        />,
      );

      expect(screen.getByRole('tooltip').textContent).toContain('Door');
    });

    it('should stay closed and tell the caller when the pointer enters', () => {
      const onOpen = vi.fn();
      const { container } = render(
        <InfoTooltip
          text="Door"
          open={false}
          onOpen={onOpen}
        />,
      );

      pointerEnters(container.firstElementChild!);

      expect(screen.queryByRole('tooltip')).toBeNull();
      expect(onOpen).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a position is given', () => {
    beforeEach(() => {
      vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
        this: Element,
      ) {
        return this.getAttribute('role') === 'tooltip'
          ? rect(0, 0, 100, 30)
          : rect(400, 300, 20, 20);
      });
    });

    it('should place the tooltip to the right of the trigger by default', () => {
      const { container } = render(<InfoTooltip text="Sets the size." />);

      pointerEnters(container.firstElementChild!);

      expect(screen.getByRole('tooltip').style.left).toBe('432px');
      expect(screen.getByRole('tooltip').style.top).toBe('295px');
    });

    it('should place a top center tooltip above the trigger', () => {
      const { container } = render(
        <InfoTooltip
          text="Sets the size."
          position="top center"
        />,
      );

      pointerEnters(container.firstElementChild!);

      expect(screen.getByRole('tooltip').style.left).toBe('360px');
      expect(screen.getByRole('tooltip').style.top).toBe('258px');
    });
  });
});
