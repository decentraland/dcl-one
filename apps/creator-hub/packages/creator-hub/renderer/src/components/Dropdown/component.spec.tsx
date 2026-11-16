import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TestProviders } from '../../../tests/utils/TestProviders';
import { Dropdown } from '.';

describe('Dropdown', () => {
  afterEach(() => {
    cleanup();
    document.documentElement.removeAttribute('style');
  });

  describe('when rendered', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[{ text: 'Duplicate', handler: () => {} }]}
          />
        </TestProviders>,
      );
    });

    it('should name the trigger and announce a menu', () => {
      const trigger = screen.getByRole('button', { name: 'Options for Plaza' });

      expect(trigger.getAttribute('aria-haspopup')).toBe('menu');
    });
  });

  describe('when the menu is opened', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[
              { text: 'Duplicate', handler: () => {}, divider: true },
              { text: 'Delete', handler: () => {} },
            ]}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should name the menu after its trigger', () => {
      expect(screen.getByRole('menu', { name: 'Options for Plaza' })).toBeTruthy();
    });

    it('should hand focus back to the trigger when Escape closes it', async () => {
      fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });

      await waitFor(() =>
        expect(document.activeElement).toBe(
          screen.getByRole('button', { name: 'Options for Plaza' }),
        ),
      );
    });

    it('should show the options', () => {
      expect(screen.getByRole('menuitem', { name: 'Duplicate' })).toBeTruthy();
    });

    it('should mark the divider as one separator between the options', () => {
      const separators = screen.getAllByRole('separator');

      expect(separators).toHaveLength(1);
      expect(separators[0].previousElementSibling?.textContent).toBe('Duplicate');
    });

    it('should leave the page scrollbar alone, so the layout around the card does not reflow', () => {
      expect(document.body.style.overflow).not.toBe('hidden');
      expect(document.body.style.paddingRight).toBe('');
    });
  });

  describe('when only some options carry a glyph', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[
              { text: 'Copy URL', handler: () => {}, icon: <svg /> },
              { text: 'Permissions', handler: () => {} },
            ]}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should give the glyph-less option an empty slot, so every label starts at the same x', () => {
      const permissions = screen.getByRole('menuitem', { name: 'Permissions' });

      expect(permissions.querySelector('.MenuGlyph')?.childElementCount).toBe(0);
    });
  });

  describe('when no option carries a glyph', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[{ text: 'Unpublish', handler: () => {} }]}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should render no glyph slot at all', () => {
      expect(
        screen.getByRole('menuitem', { name: 'Unpublish' }).querySelector('.MenuGlyph'),
      ).toBeNull();
    });
  });

  describe('when it is the filled header trigger', () => {
    const WHITE_10 = 'rgba(255, 255, 255, 0.1)';
    const paint = (color: string) => {
      const probe = document.createElement('div');
      probe.style.backgroundColor = color;
      document.body.appendChild(probe);
      const value = getComputedStyle(probe).backgroundColor;
      probe.remove();
      return value;
    };

    beforeEach(() => {
      document.documentElement.style.setProperty('--palette-whiteTransparent-subtle', WHITE_10);
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            variant="filled"
            options={[{ text: 'Copy URL', handler: () => {} }]}
          />
        </TestProviders>,
      );
    });

    it('should have no fill at rest, and White 10 while its menu is open', () => {
      const trigger = screen.getByRole('button', { name: 'Options for Plaza' });
      const white10 = paint(WHITE_10);

      expect(getComputedStyle(trigger).backgroundColor).not.toBe(white10);

      fireEvent.click(trigger);

      expect(getComputedStyle(trigger).backgroundColor).toBe(white10);
    });
  });

  describe('when an option has the danger tone', () => {
    const ERROR = 'rgb(251, 59, 59)';
    const computedColor = (color: string) => {
      const probe = document.createElement('div');
      probe.style.color = color;
      document.body.appendChild(probe);
      const value = getComputedStyle(probe).color;
      probe.remove();
      return value;
    };

    beforeEach(() => {
      document.documentElement.style.setProperty('--error', ERROR);
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[
              { text: 'Copy URL', handler: () => {}, icon: <svg data-testid="plain-glyph" /> },
              {
                text: 'Unpublish',
                handler: () => {},
                tone: 'danger',
                icon: <svg data-testid="danger-glyph" />,
              },
            ]}
          />
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should paint its glyph and label in the error colour, leaving the other items alone', () => {
      const error = computedColor(ERROR);

      expect(getComputedStyle(screen.getByTestId('danger-glyph')).color).toBe(error);
      expect(getComputedStyle(screen.getByRole('menuitem', { name: 'Unpublish' })).color).toBe(
        error,
      );
      expect(getComputedStyle(screen.getByTestId('plain-glyph')).color).not.toBe(error);
      expect(getComputedStyle(screen.getByRole('menuitem', { name: 'Copy URL' })).color).not.toBe(
        error,
      );
    });
  });

  describe('when the menu is open over a card that opens on a click', () => {
    let openCard: ReturnType<typeof vi.fn<() => void>>;

    beforeEach(() => {
      openCard = vi.fn<() => void>();
      render(
        <TestProviders>
          <div onClick={openCard}>
            <Dropdown
              label="Options for Plaza"
              options={[{ text: 'Duplicate', handler: () => {} }]}
            />
          </div>
        </TestProviders>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should close the menu on a press outside it and keep that press from the card', () => {
      // The layer with no paint over the page, before the surface of the menu.
      const backdrop = screen.getByRole('menu').parentElement
        ?.previousElementSibling as HTMLElement;
      expect(backdrop.getAttribute('aria-hidden')).toBe('true');

      fireEvent.pointerDown(backdrop);
      fireEvent.mouseDown(backdrop);
      fireEvent.click(backdrop);

      expect(screen.queryByRole('menu')).toBeNull();
      expect(openCard).not.toHaveBeenCalled();
    });

    it('should close the menu on a second click of the trigger', () => {
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));

      expect(screen.queryByRole('menu')).toBeNull();
      expect(openCard).not.toHaveBeenCalled();
    });
  });

  describe('when the menu is opened with the mouse after earlier keyboard use', () => {
    beforeEach(() => {
      render(
        <TestProviders>
          <Dropdown
            label="Options for Plaza"
            options={[
              { text: 'Duplicate', handler: () => {} },
              { text: 'Delete', handler: () => {} },
            ]}
          />
        </TestProviders>,
      );
      fireEvent.keyDown(document.body, { key: 'Tab' });
      fireEvent.click(screen.getByRole('button', { name: 'Options for Plaza' }));
    });

    it('should highlight no row until an arrow key moves', () => {
      const menu = screen.getByRole('menu').parentElement;

      expect(menu?.classList.contains('ArrowKeyed')).toBe(false);
      fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Duplicate' }), { key: 'ArrowDown' });
      expect(menu?.classList.contains('ArrowKeyed')).toBe(true);
    });
  });
});
