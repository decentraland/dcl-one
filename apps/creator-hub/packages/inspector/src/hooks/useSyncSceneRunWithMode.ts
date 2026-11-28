import { useEffect } from 'react';

import { useAppDispatch, useAppSelector } from '../redux/hooks';
import { getHiddenPanels, getSceneRunIntent, setSceneRunIntent } from '../redux/ui';
import { PanelName } from '../redux/ui/types';
import { useSdk } from './sdk/useSdk';

/**
 * Freezes the Bevy scene on entering 2D, resumes it on returning to 3D per `sceneRunIntent`,
 * and clears the run state when the renderer stops the scene itself.
 */
export function useSyncSceneRunWithMode(): void {
  const hiddenPanels = useAppSelector(getHiddenPanels);
  const isUIDesigner = !hiddenPanels[PanelName.UI_DESIGNER];
  const runIntent = useAppSelector(getSceneRunIntent);
  const sdk = useSdk();
  const dispatch = useAppDispatch();

  useEffect(
    () =>
      sdk?.renderer.sceneRun?.onStop(() =>
        dispatch(setSceneRunIntent({ running: false, reset: true })),
      ),
    [dispatch, sdk],
  );

  useEffect(() => {
    const sceneRun = sdk?.renderer.sceneRun;
    if (!sceneRun) return;
    if (isUIDesigner) {
      if (sceneRun.isRunning()) sceneRun.setRunning(false);
    } else if (runIntent && !sceneRun.isRunning()) {
      sceneRun.setRunning(true);
    }
  }, [isUIDesigner, runIntent, sdk]);
}
