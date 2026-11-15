import { createRef } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CloseIcon, InfoOutlinedIcon, WarningAmberIcon } from '.';

/** Adds one rule to the page, as a renderer stylesheet does. Returns the function that removes it. */
function addRule(rule: string) {
  const style = document.createElement('style');
  style.textContent = rule;
  document.head.append(style);
  return () => style.remove();
}

describe('Material icons', () => {
  beforeEach(() => {
    document.documentElement.style.setProperty('--brand', 'rgb(255, 45, 85)');
    document.documentElement.style.setProperty('--icon-secondary', 'rgb(255, 255, 255)');
  });

  afterEach(() => {
    cleanup();
    document.documentElement.style.removeProperty('--brand');
    document.documentElement.style.removeProperty('--icon-secondary');
  });

  describe('when rendering an icon with no props', () => {
    it('should render a decorative 24x24 svg with the icon test id', () => {
      render(<CloseIcon />);
      const svg = screen.getByTestId('CloseIcon');
      expect(svg.tagName.toLowerCase()).toBe('svg');
      expect(svg.getAttribute('viewBox')).toBe('0 0 24 24');
      expect(svg.getAttribute('aria-hidden')).toBe('true');
      expect(svg.getAttribute('focusable')).toBe('false');
      expect(svg.getAttribute('role')).toBeNull();
    });

    it('should render at 24px and fill with the current colour', () => {
      render(<CloseIcon />);
      const style = getComputedStyle(screen.getByTestId('CloseIcon'));
      expect(style.fontSize).toBe('24px');
      expect(style.fill.toLowerCase()).toBe('currentcolor');
    });

    it('should draw the path of the Material icon', () => {
      render(<CloseIcon />);
      const paths = screen.getByTestId('CloseIcon').querySelectorAll('path');
      expect(paths).toHaveLength(1);
      expect(paths[0].getAttribute('d')).toBe(
        'M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z',
      );
    });
  });

  describe('when the icon has two paths', () => {
    it('should render both paths', () => {
      render(<WarningAmberIcon />);
      expect(screen.getByTestId('WarningAmberIcon').querySelectorAll('path')).toHaveLength(2);
    });
  });

  describe('when the fontSize prop is set', () => {
    it.each([
      ['small', '20px'],
      ['medium', '24px'],
      ['large', '35px'],
    ] as const)('should render %s at %s', (fontSize, expected) => {
      render(<CloseIcon fontSize={fontSize} />);
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).fontSize).toBe(expected);
    });

    it('should take the size of the parent for inherit', () => {
      render(
        <span style={{ fontSize: '13px' }}>
          <CloseIcon fontSize="inherit" />
        </span>,
      );
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).fontSize).toBe('13px');
    });
  });

  describe('when the color prop is set', () => {
    it('should use the secondary icon colour', () => {
      render(<CloseIcon color="secondary" />);
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).color).toBe('rgb(255, 255, 255)');
    });

    it('should use the brand colour for primary', () => {
      render(<CloseIcon color="primary" />);
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).color).toBe('rgb(255, 45, 85)');
    });
  });

  describe('when the htmlColor prop is set', () => {
    it('should set the color attribute', () => {
      render(<CloseIcon htmlColor="#ff0000" />);
      expect(screen.getByTestId('CloseIcon').getAttribute('color')).toBe('#ff0000');
    });
  });

  describe('when the sx prop is set', () => {
    it('should apply a number font size in pixels', () => {
      render(<CloseIcon sx={{ fontSize: 13 }} />);
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).fontSize).toBe('13px');
    });

    it('should apply the margin and the visibility', () => {
      render(<CloseIcon sx={{ marginLeft: 'auto', visibility: 'hidden' }} />);
      const style = getComputedStyle(screen.getByTestId('CloseIcon'));
      expect(style.marginLeft).toBe('auto');
      expect(style.visibility).toBe('hidden');
    });

    it('should keep the style prop of the caller', () => {
      render(
        <CloseIcon
          sx={{ fontSize: 16 }}
          style={{ opacity: 0.5 }}
        />,
      );
      const svg = screen.getByTestId('CloseIcon');
      expect(svg.style.opacity).toBe('0.5');
      expect(getComputedStyle(svg).fontSize).toBe('16px');
    });
  });

  describe('when the titleAccess prop is set', () => {
    it('should expose the icon as an image with that name', () => {
      render(<InfoOutlinedIcon titleAccess="Pinned" />);
      const svg = screen.getByRole('img', { name: 'Pinned' });
      expect(svg.getAttribute('aria-hidden')).toBeNull();
      expect(svg.querySelector('title')?.textContent).toBe('Pinned');
    });
  });

  describe('when the caller passes other props', () => {
    it('should take the rules for the class name of the caller and keep the base look', () => {
      const removeRule = addRule('.offline-icon { opacity: 0.25; }');
      render(<CloseIcon className="offline-icon" />);
      const style = getComputedStyle(screen.getByTestId('CloseIcon'));
      expect(style.opacity).toBe('0.25');
      expect(style.fontSize).toBe('24px');
      expect(style.fill.toLowerCase()).toBe('currentcolor');
      removeRule();
    });

    it('should take the rules that the renderer stylesheets write for every icon', () => {
      const removeRule = addRule('.HomeCardIcon .MuiSvgIcon-root { opacity: 0.5; }');
      render(
        <span className="HomeCardIcon">
          <CloseIcon className="offline-icon" />
        </span>,
      );
      expect(getComputedStyle(screen.getByTestId('CloseIcon')).opacity).toBe('0.5');
      removeRule();
    });

    it('should call onClick', () => {
      const onClick = vi.fn();
      render(<CloseIcon onClick={onClick} />);
      fireEvent.click(screen.getByTestId('CloseIcon'));
      expect(onClick).toHaveBeenCalledTimes(1);
    });

    it('should forward the ref and the attributes to the svg element', () => {
      const ref = createRef<SVGSVGElement>();
      render(
        <CloseIcon
          ref={ref}
          tabIndex={0}
          aria-label="More"
        />,
      );
      const svg = screen.getByTestId('CloseIcon');
      expect(ref.current).toBe(svg);
      expect(svg.getAttribute('tabindex')).toBe('0');
      expect(svg.getAttribute('aria-label')).toBe('More');
    });

    it('should let the caller replace the test id', () => {
      render(<CloseIcon data-testid="close" />);
      expect(screen.getByTestId('close')).toBeTruthy();
      expect(screen.queryByTestId('CloseIcon')).toBeNull();
    });
  });
});
