import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import type { Entity } from '@dcl/ecs';

import { YGU_AUTO, YGU_PERCENT, YGU_POINT } from '../../../../../lib/sdk/ui-transform-constants';
import type { FieldConfig } from '../field-configs';
import { ResizeField } from './ResizeField';

vi.mock('../BindableSubField', () => ({
  BindableSubField: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock('../../../shared/measure', async importOriginal => ({
  ...(await importOriginal<object>()),
  measureNodeBox: () => ({ width: 120.4, height: 30 }),
}));
vi.mock('../../../../../redux/hooks', () => ({
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({ uiDesigner: { aspectLocked: {} } }),
  useAppDispatch: () => vi.fn(),
}));

const field = {
  componentId: 'uiTransform',
  path: '',
  kind: 'resize',
  aspectLockable: true,
  subFields: [
    { path: 'width', leftLabel: 'W' },
    { path: 'height', leftLabel: 'H' },
  ],
} as unknown as FieldConfig;

const widthMode = (value: Record<string, unknown>) => {
  render(
    <ResizeField
      field={field}
      value={value}
      entity={1 as Entity}
      parentFlexDirection={0}
      onPatch={vi.fn()}
    />,
  );
  return screen.getByLabelText('Width resize mode');
};

describe('ResizeField mode display', () => {
  it.each([
    ['a fixed size', { width: 100, widthUnit: YGU_POINT }, 'px'],
    ['a percent size', { width: 50, widthUnit: YGU_PERCENT }, '%'],
    ['a hug size', { widthUnit: YGU_AUTO }, 'Hug'],
  ])('should show %s as %j', (_, value, shown) => {
    expect(widthMode(value).textContent).toBe(shown);
  });

  it('should never show the word Fixed in the trigger', () => {
    expect(widthMode({ width: 100 }).textContent).not.toContain('Fixed');
  });

  describe('when the mode list opens', () => {
    it("should keep the code's mode labels", () => {
      fireEvent.click(widthMode({ width: 100, widthUnit: YGU_POINT }));
      const list = screen.getByRole('listbox', { hidden: true });
      expect(within(list).getByText('Fixed')).toBeTruthy();
      expect(within(list).getByText('Percent')).toBeTruthy();
      expect(within(list).getByText('Hug')).toBeTruthy();
    });
  });

  describe.each([
    ['hug', { widthUnit: YGU_AUTO }],
    ['fill', { flexGrow: 1 }],
  ])('when the width is %s', (_, value) => {
    it('should show the rendered size as a read-only, enabled value', () => {
      widthMode(value);
      const input = screen.getAllByRole('spinbutton', { hidden: true })[0] as HTMLInputElement;
      expect(input.value).toBe('120');
      expect(input.readOnly).toBe(true);
      expect(input.disabled).toBe(false);
    });
  });
});
