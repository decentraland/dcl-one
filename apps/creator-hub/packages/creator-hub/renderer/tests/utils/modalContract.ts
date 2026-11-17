import { fireEvent, screen } from '@testing-library/react';
import { expect } from 'vitest';

const STOPS =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function pressEscape() {
  fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
}

/** A press that starts and ends on the backdrop, the element that holds the dialog. */
export function pressBackdrop() {
  const backdrop = screen.getByRole('dialog').parentElement!;
  fireEvent.pointerDown(backdrop);
  fireEvent.click(backdrop);
}

/** Focus is in the dialog; Tab on the last control goes to the first and Shift+Tab goes back. */
export function expectFocusTrap() {
  const dialog = screen.getByRole('dialog');
  const stops = Array.from(dialog.querySelectorAll<HTMLElement>(STOPS)).filter(
    stop => stop.tabIndex >= 0,
  );
  const first = stops[0];
  const last = stops[stops.length - 1];

  expect(dialog.getAttribute('aria-modal')).toBe('true');
  expect(dialog.contains(document.activeElement)).toBe(true);
  expect(stops.length).toBeGreaterThan(0);
  last.focus();
  fireEvent.keyDown(last, { key: 'Tab' });
  expect(document.activeElement).toBe(first);
  fireEvent.keyDown(first, { key: 'Tab', shiftKey: true });
  expect(document.activeElement).toBe(last);
}
