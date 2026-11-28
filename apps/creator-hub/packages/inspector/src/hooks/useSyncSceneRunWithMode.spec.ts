import { describe, expect, it, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';

import { setSceneRunIntent } from '../redux/ui';
import { PanelName } from '../redux/ui/types';
import { useSyncSceneRunWithMode } from './useSyncSceneRunWithMode';

const mocks = vi.hoisted(() => ({
  hiddenPanels: {} as Record<string, boolean>,
  runIntent: false,
  sceneRun: null as {
    isRunning: () => boolean;
    setRunning: (running: boolean) => void;
    onStop: (cb: () => void) => () => void;
  } | null,
  setRunning: vi.fn(),
  dispatch: vi.fn(),
  stopListeners: new Set<() => void>(),
  running: false,
}));

vi.mock('../redux/hooks', () => ({
  useAppDispatch: () => mocks.dispatch,
  useAppSelector: (selector: (s: unknown) => unknown) =>
    selector({ ui: { hiddenPanels: mocks.hiddenPanels, sceneRunIntent: mocks.runIntent } }),
}));

vi.mock('./sdk/useSdk', () => ({
  useSdk: () => ({ renderer: { sceneRun: mocks.sceneRun } }),
}));

beforeEach(() => {
  mocks.setRunning.mockClear();
  mocks.dispatch.mockClear();
  mocks.stopListeners.clear();
  mocks.running = false;
  mocks.runIntent = false;
  mocks.sceneRun = {
    isRunning: () => mocks.running,
    setRunning: mocks.setRunning,
    onStop: cb => {
      mocks.stopListeners.add(cb);
      return () => mocks.stopListeners.delete(cb);
    },
  };
  mocks.hiddenPanels = { [PanelName.UI_DESIGNER]: true };
});

describe('when the UI Designer opens', () => {
  it('should freeze a running scene', () => {
    mocks.running = true;
    mocks.hiddenPanels = { [PanelName.UI_DESIGNER]: false };

    renderHook(() => useSyncSceneRunWithMode());

    expect(mocks.setRunning).toHaveBeenCalledWith(false);
  });

  it('should leave an already frozen scene alone', () => {
    mocks.hiddenPanels = { [PanelName.UI_DESIGNER]: false };

    renderHook(() => useSyncSceneRunWithMode());

    expect(mocks.setRunning).not.toHaveBeenCalled();
  });

  it('should not throw on a renderer without a run state', () => {
    mocks.sceneRun = null;
    mocks.hiddenPanels = { [PanelName.UI_DESIGNER]: false };

    expect(() => renderHook(() => useSyncSceneRunWithMode())).not.toThrow();
  });
});

describe('when the 3D viewport is showing', () => {
  it('should resume the scene when the run intent was set', () => {
    mocks.runIntent = true;

    renderHook(() => useSyncSceneRunWithMode());

    expect(mocks.setRunning).toHaveBeenCalledWith(true);
  });

  it('should not resume when there is no run intent', () => {
    renderHook(() => useSyncSceneRunWithMode());

    expect(mocks.setRunning).not.toHaveBeenCalled();
  });

  it('should not re-post a resume when the scene is already running', () => {
    mocks.runIntent = true;
    mocks.running = true;

    renderHook(() => useSyncSceneRunWithMode());

    expect(mocks.setRunning).not.toHaveBeenCalled();
  });
});

describe('when the renderer stops the scene itself', () => {
  it('should clear the run intent and the started state', () => {
    renderHook(() => useSyncSceneRunWithMode());

    mocks.stopListeners.forEach(listener => listener());

    expect(mocks.dispatch).toHaveBeenCalledWith(setSceneRunIntent({ running: false, reset: true }));
  });

  it('should stop listening on unmount', () => {
    const { unmount } = renderHook(() => useSyncSceneRunWithMode());

    unmount();

    expect(mocks.stopListeners.size).toBe(0);
  });
});
