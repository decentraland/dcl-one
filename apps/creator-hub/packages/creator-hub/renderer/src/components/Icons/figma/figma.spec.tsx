import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { Edit, Leave } from '.';

function renderSvg(ui: React.ReactElement) {
  const { container } = render(ui);
  return container.querySelector('svg')!;
}

describe('Creator Hub icon binding', () => {
  afterEach(cleanup);

  describe('when rendering an icon at the default size', () => {
    it('should render the 24 drawing, decorative', () => {
      const svg = renderSvg(<Edit />);
      expect(svg.getAttribute('viewBox')).toBe('0 0 24 24');
      expect(svg.getAttribute('overflow')).toBe('visible');
      expect(svg.getAttribute('aria-hidden')).toBe('true');
    });
  });

  describe('when rendering an icon with fontSize small', () => {
    it('should render the small drawing at 16px', () => {
      const base = renderSvg(<Edit />).innerHTML;
      cleanup();
      const svg = renderSvg(<Edit fontSize="small" />);
      expect(svg.getAttribute('viewBox')).toBe('4 4 16 16');
      expect(svg.innerHTML).not.toBe(base);
      expect(svg.getAttribute('overflow')).toBeNull();
      expect(getComputedStyle(svg).fontSize).toBe('16px');
    });

    describe('and the icon has no small drawing', () => {
      it('should keep the 24 drawing at 16px', () => {
        const svg = renderSvg(<Leave fontSize="small" />);
        expect(svg.getAttribute('viewBox')).toBe('0 0 24 24');
        expect(getComputedStyle(svg).fontSize).toBe('16px');
      });
    });
  });

  describe('when rendering an icon with the primary tone', () => {
    it('should paint it --brand', () => {
      document.documentElement.style.setProperty('--brand', 'rgb(255, 45, 85)');
      const svg = renderSvg(<Edit tone="primary" />);
      expect(getComputedStyle(svg).color).toBe('rgb(255, 45, 85)');
      document.documentElement.style.removeProperty('--brand');
    });
  });

  describe('when rendering an icon with a className', () => {
    it('should keep its own class beside the call site one', () => {
      const svg = renderSvg(<Edit className="extra" />);
      expect(svg.classList.contains('extra')).toBe(true);
      expect(svg.classList.length).toBe(2);
    });
  });

  describe('when rendering an icon with a title', () => {
    it('should expose a named image and drop aria-hidden', () => {
      const svg = renderSvg(<Edit title="Edit" />);
      expect(svg.querySelector('title')?.textContent).toBe('Edit');
      expect(svg.getAttribute('role')).toBe('img');
      expect(svg.getAttribute('aria-label')).toBe('Edit');
      expect(svg.hasAttribute('aria-hidden')).toBe(false);
    });
  });

  describe('when reading the component name', () => {
    it('should carry the Figma name', () => {
      expect(Edit.displayName).toBe('EditIcon');
    });
  });
});
