import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import type * as AnalyticsModule from '../../lib/logic/analytics';
import { Event } from '../../lib/logic/analytics';
import Toolbar from './Toolbar';

const mocks = vi.hoisted(() => ({
  track: vi.fn(),
  dispatch: vi.fn(),
  canUndo: true,
  canRedo: true,
}));

vi.mock('../../lib/logic/analytics', async importActual => ({
  ...(await importActual<typeof AnalyticsModule>()),
  analytics: { track: mocks.track },
}));

vi.mock('../../redux/hooks', () => ({
  useAppDispatch: () => mocks.dispatch,
  useAppSelector: (selector: (state: unknown) => unknown) =>
    selector({
      app: { canSave: false, preferences: { autosaveEnabled: true } },
      ui: { sceneRunStarted: false },
      dataLayer: {
        undoRedoState: { canUndo: mocks.canUndo, canRedo: mocks.canRedo },
        sceneInfo: { content: '' },
      },
    }),
}));

vi.mock('../../hooks/sdk/useSdk', () => ({
  useSdk: () => ({ renderer: {}, operations: {}, engine: {} }),
}));

vi.mock('../../hooks/sdk/useInspectorUIState', () => ({
  useInspectorUIState: () => [{}, vi.fn()],
}));

vi.mock('./Gizmos', () => ({ Gizmos: () => null }));

const pressCtrl = (key: string, options: { shiftKey?: boolean } = {}) =>
  fireEvent.keyDown(document, {
    key,
    keyCode: key.toUpperCase().charCodeAt(0),
    ctrlKey: true,
    ...options,
  });

describe('Toolbar undo and redo tracking', () => {
  beforeEach(() => {
    mocks.track.mockClear();
    mocks.dispatch.mockClear();
    mocks.canUndo = true;
    mocks.canRedo = true;
  });

  afterEach(cleanup);

  describe('when the undo button is clicked', () => {
    beforeEach(() => {
      render(<Toolbar />);
      fireEvent.click(screen.getByTitle('Undo'));
    });

    it('should track an Undo from the toolbar', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.UNDO, { source: 'toolbar' });
    });
  });

  describe('when the redo button is clicked', () => {
    beforeEach(() => {
      render(<Toolbar />);
      fireEvent.click(screen.getByTitle('Redo'));
    });

    it('should track a Redo from the toolbar', () => {
      expect(mocks.track).toHaveBeenCalledWith(Event.REDO, { source: 'toolbar' });
    });
  });

  describe('when the undo shortcut is pressed', () => {
    beforeEach(() => {
      render(<Toolbar />);
      pressCtrl('z');
    });

    it('should track one Undo from the shortcut', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.UNDO, { source: 'shortcut' });
    });
  });

  describe('when the redo shortcut is pressed', () => {
    beforeEach(() => {
      render(<Toolbar />);
      pressCtrl('y');
    });

    it('should track one Redo from the shortcut', () => {
      expect(mocks.track).toHaveBeenCalledTimes(1);
      expect(mocks.track).toHaveBeenCalledWith(Event.REDO, { source: 'shortcut' });
    });
  });

  describe('when the undo shortcut is pressed with nothing to undo', () => {
    beforeEach(() => {
      mocks.canUndo = false;
      render(<Toolbar />);
      pressCtrl('z');
    });

    it('should still dispatch the undo', () => {
      expect(mocks.dispatch).toHaveBeenCalled();
    });

    it('should not track an Undo', () => {
      expect(mocks.track).not.toHaveBeenCalled();
    });
  });
});
