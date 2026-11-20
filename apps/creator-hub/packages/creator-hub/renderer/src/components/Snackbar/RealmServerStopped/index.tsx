import { useCallback } from 'react';
import Button from '@ui/atoms/Button';

import { RENDERER } from '/shared/types/settings';

import { t } from '/@/modules/store/translation/utils';
import { useEditor } from '/@/hooks/useEditor';
import { useSettings } from '/@/hooks/useSettings';
import { editor } from '#preload';

import { SnackbarAlert } from '../Alert';

// The editor realm's Multiplayer Server kept stopping and Creator Hub stopped restarting it.
// The editor keeps working without it; Retry replaces the realm, which reloads the editor.
export function RealmServerStopped({ path, onClose }: { path: string; onClose: () => void }) {
  const { project } = useEditor();
  const { settings } = useSettings();
  // Only the open Bevy editor of the scene owns its realm; a realm started for a scene the
  // user has left, or under another renderer, would run with nothing to stop it.
  const editingScene = project?.path === path && settings.renderer === RENDERER.BEVY;

  const handleRetry = useCallback(() => {
    onClose();
    // A failure reaches the editor as a stopped-realm event, which shows it.
    if (editingScene) editor.ensureBevyRealm(path).catch(() => {});
  }, [path, onClose, editingScene]);

  return (
    <SnackbarAlert
      severity="error"
      data-testid="snackbar-realm-server-stopped"
      action={
        <>
          <Button
            variant="text"
            onClick={onClose}
          >
            {t('snackbar.realm_server_stopped.actions.dismiss')}
          </Button>
          <Button
            variant="primary"
            onClick={handleRetry}
          >
            {t('snackbar.realm_server_stopped.actions.retry')}
          </Button>
        </>
      }
    >
      {t('snackbar.realm_server_stopped.title')}
    </SnackbarAlert>
  );
}
