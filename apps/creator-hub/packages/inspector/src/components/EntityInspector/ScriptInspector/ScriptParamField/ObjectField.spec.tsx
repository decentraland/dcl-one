import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';

import type { ScriptParamObject, ScriptParamUnion } from '../types';
import { ObjectField } from './ObjectField';
import { formatLabel } from './labels';

vi.mock('./ScriptParamField', () => ({
  ScriptParamField: ({ name, param }: { name: string; param: ScriptParamUnion }) =>
    param.type === 'object' ? (
      <ObjectField
        name={name}
        param={param}
        onUpdate={() => {}}
      />
    ) : (
      <span>{formatLabel(name)}</span>
    ),
}));

const param: ScriptParamObject = {
  type: 'object',
  value: { isEnabled: true, kickCoordinates: { x: 1 } },
  fields: {
    isEnabled: { type: 'boolean', value: false },
    kickCoordinates: {
      type: 'object',
      value: { x: 0 },
      fields: { x: { type: 'number', value: 0 } },
    },
  },
};

const renderField = (overrides: Partial<ScriptParamObject> = {}) =>
  render(
    <ObjectField
      name="moderationControl"
      param={{ ...param, ...overrides }}
      onUpdate={vi.fn()}
    />,
  );

const header = () => screen.getByRole('button', { name: 'Moderation Control' });

describe('when an object param renders', () => {
  it('should draw its group as a collapsed accordion header with the body hidden', () => {
    renderField();

    expect(header().closest('.Accordion')).not.toBeNull();
    expect(header().getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('Is Enabled')).toBeNull();
  });

  it('should keep an isEnabled member as an ordinary field, with no header checkbox', () => {
    renderField();

    expect(screen.queryByRole('checkbox', { name: 'Enable Moderation Control' })).toBeNull();
    expect(header().closest('.Accordion')?.classList.contains('has-toggle')).toBe(false);
  });
});

describe('when the group header is activated', () => {
  it('should hold every field in the expanded body, nested groups included', () => {
    renderField();

    fireEvent.click(header());
    const body = screen.getByRole('region', { name: 'Moderation Control' });
    const nested = within(body).getByRole('button', { name: 'Kick Coordinates' });

    expect(header().getAttribute('aria-expanded')).toBe('true');
    expect(within(body).getByText('Is Enabled')).toBeTruthy();
    expect(nested.getAttribute('aria-expanded')).toBe('false');

    fireEvent.click(nested);

    expect(within(body).getByText('X')).toBeTruthy();
  });

  it('should collapse the body again on a second activation', () => {
    renderField();

    fireEvent.click(header());
    fireEvent.click(header());

    expect(header().getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText('Is Enabled')).toBeNull();
  });
});

describe('when the object param has a tooltip', () => {
  it('should show it in the header, outside the toggle button', () => {
    const { container } = renderField({ tooltip: 'Kick players and manage the allow-list.' });

    const info = container.querySelector('.Accordion > .header > .info');

    expect(info).not.toBeNull();
    expect(header().contains(info)).toBe(false);
  });
});
