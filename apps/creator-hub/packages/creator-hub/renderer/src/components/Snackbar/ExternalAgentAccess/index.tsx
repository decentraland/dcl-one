import { useCallback } from 'react';
import { SmartToyOutlinedIcon } from '@ui/atoms/icons-material';
import Button from '@ui/atoms/Button';

import { t } from '/@/modules/store/translation/utils';
import { useSettings } from '/@/hooks/useSettings';

import { SnackbarAlert } from '../Alert';

// An AI agent outside the Hub asked for access to the open scene. It stays up until the
// creator answers; only "Allow" turns access on.
export function ExternalAgentAccess({ onClose }: { onClose: () => void }) {
  const { settings, updateAppSettings } = useSettings();

  const handleAllow = useCallback(() => {
    updateAppSettings({ ...settings, exposeMcpServer: true });
    onClose();
  }, [settings, updateAppSettings, onClose]);

  return (
    <SnackbarAlert
      icon={<SmartToyOutlinedIcon tone="secondary" />}
      severity="info"
      action={
        <>
          <Button
            variant="text"
            onClick={onClose}
          >
            {t('snackbar.external_agent_access.actions.deny')}
          </Button>
          <Button
            variant="primary"
            onClick={handleAllow}
          >
            {t('snackbar.external_agent_access.actions.allow')}
          </Button>
        </>
      }
    >
      {t('snackbar.external_agent_access.title')}
    </SnackbarAlert>
  );
}
