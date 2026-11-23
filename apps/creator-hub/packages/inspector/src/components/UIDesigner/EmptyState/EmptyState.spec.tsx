import { readFileSync } from 'fs';
import { join } from 'path';
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';

import { EmptyState, EmptyStateChip } from './EmptyState';

afterEach(cleanup);

const root = () => document.querySelector<HTMLElement>('.ui-designer-empty-state')!;
const title = () => document.querySelector<HTMLElement>('.ui-designer-empty-state-title')!;

describe('when an EmptyState renders as the canvas variant', () => {
  it('should carry the canvas class and title the state with an h2', () => {
    render(
      <EmptyState
        variant="canvas"
        title="Start building your UI"
      />,
    );

    expect(root().classList.contains('ui-designer-empty-state--canvas')).toBe(true);
    expect(title().tagName).toBe('H2');
    expect(title().textContent).toBe('Start building your UI');
  });
});

describe('when an EmptyState renders as the panel variant', () => {
  it('should carry the panel class and title the state with a paragraph', () => {
    render(
      <EmptyState
        variant="panel"
        title="No node selected"
        message="Select a node."
      />,
    );

    expect(root().classList.contains('ui-designer-empty-state--panel')).toBe(true);
    expect(title().tagName).toBe('P');
    expect(document.querySelector('.ui-designer-empty-state-message')!.textContent).toBe(
      'Select a node.',
    );
  });
});

describe('when an EmptyState renders as the inline variant', () => {
  it('should keep its icon, aria-hidden, and title it with a paragraph', () => {
    render(
      <EmptyState
        variant="inline"
        icon={<svg />}
        title="Device variants"
      />,
    );

    expect(root().classList.contains('ui-designer-empty-state--inline')).toBe(true);
    expect(title().tagName).toBe('P');
    expect(
      document.querySelector('.ui-designer-empty-state-icon')!.getAttribute('aria-hidden'),
    ).toBe('true');
  });
});

describe('when an EmptyStateChip names a control', () => {
  it.each(['muted', 'strong'] as const)('should carry its %s tone', tone => {
    render(<EmptyStateChip tone={tone}>Nodes +</EmptyStateChip>);
    const chip = document.querySelector('.ui-designer-empty-state-chip')!;

    expect(chip.classList.contains(`ui-designer-empty-state-chip--${tone}`)).toBe(true);
    expect(chip.textContent).toBe('Nodes +');
  });
});

describe('when the stylesheet sizes the chips', () => {
  const stylesheet = readFileSync(join(__dirname, 'EmptyState.css'), 'utf8');
  const rule = (selector: string) =>
    Array.from(stylesheet.matchAll(/([^{}]+)\{([^{}]*)\}/g))
      .find(([, sel]) => sel.trim() === selector)?.[2]
      .replace(/\s+/g, ' ');

  it.each([
    ['muted', 'padding: 0 4px;'],
    ['strong', 'padding: 3px 4px;'],
  ])('should pad the %s chip to the drawn width', (tone, padding) => {
    expect(rule(`.ui-designer-empty-state-chip--${tone}`)).toContain(padding);
  });
});
