import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { GizmoType } from '../../lib/utils/gizmo';
import type { SdkContextValue } from '../../lib/sdk/context';
import { areGizmosDisabled, getHiddenPanels } from '../../redux/ui';
import { PanelName } from '../../redux/ui/types';
import { useTransformInput } from './useTransformInput';

vi.mock('../../redux/hooks', () => ({
  useAppSelector: (selector: unknown) => {
    if (selector === areGizmosDisabled) return false;
    if (selector === getHiddenPanels) return { [PanelName.UI_DESIGNER]: true };
    return undefined;
  },
}));
vi.mock('../sdk/useEntitiesWith', () => ({ useEntitiesWith: () => SELECTED }));

const SELECTED = [512];
const updateValue = vi.fn();

const sdk = {
  components: {
    Selection: { getOrNull: () => ({ gizmo: GizmoType.FREE }) },
    Transform: { getOrNull: () => null },
  },
  operations: { updateValue, dispatch: vi.fn() },
  renderer: {
    spawnPoints: { getSelectedIndex: () => null, onSelectionChange: () => () => {} },
    events: { on: vi.fn(), off: vi.fn() },
  },
} as unknown as SdkContextValue;

const press = (key: string) =>
  document.body.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));

const gizmosWritten = () => updateValue.mock.calls.map(([, , value]) => value.gizmo);

const openMenu = () => {
  const menu = document.createElement('div');
  menu.setAttribute('role', 'menu');
  document.body.appendChild(menu);
  return menu;
};

describe('useTransformInput', () => {
  beforeEach(() => {
    updateValue.mockClear();
  });
  afterEach(() => {
    document.body.replaceChildren();
  });

  describe('when no context menu is open', () => {
    it.each([
      ['g', GizmoType.POSITION],
      ['r', GizmoType.ROTATION],
      ['x', GizmoType.SCALE],
    ])('should switch the gizmo on %s', (key, gizmo) => {
      renderHook(() => useTransformInput(sdk));
      press(key);
      expect(gizmosWritten()).toEqual([gizmo]);
    });
  });

  describe('when a context menu is open', () => {
    it.each(['g', 'r', 'x'])('should leave %s to the menu', key => {
      openMenu();
      renderHook(() => useTransformInput(sdk));
      press(key);
      expect(updateValue).not.toHaveBeenCalled();
    });

    it('should switch the gizmo again once the menu closes', () => {
      const menu = openMenu();
      renderHook(() => useTransformInput(sdk));
      menu.remove();
      press('g');
      expect(gizmosWritten()).toEqual([GizmoType.POSITION]);
    });
  });
});
