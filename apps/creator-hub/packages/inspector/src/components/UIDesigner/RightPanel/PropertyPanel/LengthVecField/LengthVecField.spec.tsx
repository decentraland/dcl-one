import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { Entity } from '@dcl/ecs';

import { YGU_PERCENT } from '../../../../../lib/sdk/ui-transform-constants';
import type { FieldConfig } from '../field-configs';
import { LengthVecField } from './LengthVecField';

vi.mock('../BindableField', () => ({
  BindableField: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock('../BindableSubField', () => ({
  BindableSubField: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock('../../../shared/measure', async importOriginal => ({
  ...(await importOriginal<object>()),
  measureParentBox: () => ({ width: 200, height: 50 }),
}));
vi.mock('../../../../../redux/hooks', () => ({
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({ uiDesigner: { aspectLocked: {} } }),
  useAppDispatch: () => vi.fn(),
}));

const field = {
  componentId: 'uiTransform',
  path: 'size',
  kind: 'lengthVec',
  aspectLockable: true,
  subFields: [
    { path: 'width', leftLabel: 'W' },
    { path: 'height', leftLabel: 'H' },
  ],
} as unknown as FieldConfig;

const lockOf = (fieldDisabled: boolean) =>
  render(
    <LengthVecField
      field={field}
      componentValue={{ width: 10, height: 10 }}
      entity={1 as Entity}
      fieldDisabled={fieldDisabled}
      onPatch={vi.fn()}
    />,
  ).getByLabelText('Lock aspect ratio') as HTMLButtonElement;

describe('LengthVecField aspect lock', () => {
  describe('when the field is disabled', () => {
    it('should disable the aspect lock', () => {
      expect(lockOf(true).disabled).toBe(true);
    });
  });

  describe('when the field is enabled', () => {
    it('should leave the aspect lock enabled', () => {
      expect(lockOf(false).disabled).toBe(false);
    });
  });
});

const renderUnits = (
  componentValue: Record<string, unknown>,
  { fieldDisabled = false, onPatch = vi.fn() } = {},
) => {
  render(
    <LengthVecField
      field={field}
      componentValue={componentValue}
      entity={1 as Entity}
      fieldDisabled={fieldDisabled}
      onPatch={onPatch}
    />,
  );
  return {
    onPatch,
    units: () => screen.getAllByLabelText(/unit/i),
    inputs: () => screen.getAllByRole('spinbutton', { hidden: true }) as HTMLInputElement[],
  };
};

const percent = { width: 50, height: 25, widthUnit: YGU_PERCENT, heightUnit: YGU_PERCENT };

describe('LengthVecField units', () => {
  describe('when the field renders', () => {
    it('should put a unit control inside each cell, after its number', () => {
      const { units, inputs } = renderUnits({ width: 10, height: 20 });
      expect(units()).toHaveLength(2);
      units().forEach((unit, index) => {
        const cell = inputs()[index].closest('.InputContainer');
        expect(cell?.contains(unit)).toBe(true);
        expect(inputs()[index].compareDocumentPosition(unit)).toBe(
          Node.DOCUMENT_POSITION_FOLLOWING,
        );
      });
    });
  });

  describe('when both axes are in %', () => {
    it('should show % in both unit controls', () => {
      const { units } = renderUnits(percent);
      units().forEach(unit => expect(within(unit).getByText('%')).toBeTruthy());
    });

    it('should keep the value in % after an edit', async () => {
      const { inputs, onPatch } = renderUnits(percent);
      fireEvent.change(inputs()[0], { target: { value: '60' } });
      fireEvent.blur(inputs()[0]);
      await waitFor(() => expect(onPatch).toHaveBeenCalled());
      expect(onPatch.mock.lastCall?.[0]).toMatchObject({ width: 60, widthUnit: YGU_PERCENT });
    });
  });

  describe('when either unit control switches to %', () => {
    it('should switch both axes', () => {
      const { units, onPatch } = renderUnits({ width: 10, height: 20 });
      fireEvent.click(units()[1]);
      fireEvent.click(within(screen.getByRole('listbox', { hidden: true })).getByText('%'));
      expect(onPatch).toHaveBeenCalledWith(
        expect.objectContaining({ widthUnit: YGU_PERCENT, heightUnit: YGU_PERCENT }),
      );
    });
  });

  describe('when the field is disabled', () => {
    it('should disable every unit control', () => {
      const { units } = renderUnits({ width: 10, height: 20 }, { fieldDisabled: true });
      units().forEach(unit => expect(unit.getAttribute('aria-disabled')).toBe('true'));
    });
  });
});

describe('LengthVecField mixed units', () => {
  const mixed = { width: 50, height: 40, heightUnit: YGU_PERCENT };

  describe('when X is in px and Y in %', () => {
    it('should show each cell its own unit', () => {
      const { units } = renderUnits(mixed);
      expect(units().map(unit => unit.textContent)).toEqual(['px', '%']);
    });

    it('should keep an edited cell in its own unit', async () => {
      const { inputs, onPatch } = renderUnits(mixed);
      fireEvent.change(inputs()[1], { target: { value: '30' } });
      fireEvent.blur(inputs()[1]);
      await waitFor(() => expect(onPatch).toHaveBeenCalled());
      expect(onPatch.mock.lastCall?.[0]).toMatchObject({ height: 30, heightUnit: YGU_PERCENT });
    });
  });

  describe('when % is picked in the px cell', () => {
    it('should convert that cell and leave the % cell as it is, now shared', () => {
      const { units, onPatch } = renderUnits(mixed);
      fireEvent.click(units()[0]);
      fireEvent.click(within(screen.getByRole('listbox', { hidden: true })).getByText('%'));
      expect(onPatch).toHaveBeenCalledWith({
        width: 25,
        widthUnit: YGU_PERCENT,
        height: 40,
        heightUnit: YGU_PERCENT,
      });
    });
  });
});
