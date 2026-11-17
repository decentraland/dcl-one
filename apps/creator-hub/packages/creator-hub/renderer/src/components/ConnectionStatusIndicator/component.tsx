import { useState, useCallback } from 'react';
import IconButton from '@ui/atoms/IconButton';
import Tooltip from '@ui/atoms/Tooltip';
import { SignalWifiOffIcon } from '@ui/atoms/icons-material';

import { useConnectionStatus } from '/@/hooks/useConnectionStatus';
import { ConnectionStatus } from '/@/lib/connection';
import { t } from '/@/modules/store/translation/utils';
import { ConnectionModal } from './ConnectionModal';

import './styles.css';

export function ConnectionStatusIndicator() {
  const { status } = useConnectionStatus();
  const [modalOpen, setModalOpen] = useState(false);

  const handleClick = useCallback(() => {
    setModalOpen(true);
  }, []);

  const handleClose = useCallback(() => {
    setModalOpen(false);
  }, []);

  if (status !== ConnectionStatus.OFFLINE) return null;

  return (
    <>
      <div className="ConnectionStatusIndicator">
        <Tooltip
          label={t('connection.offline.tooltip')}
          side="bottom"
          tipClassName="ConnectionStatusTip"
          portal
          keepInViewport
        >
          <IconButton
            aria-label="connection-status"
            className="connection-status-offline"
            onClick={handleClick}
          >
            <SignalWifiOffIcon className="offline-icon" />
          </IconButton>
        </Tooltip>
      </div>
      <ConnectionModal
        open={modalOpen}
        onClose={handleClose}
      />
    </>
  );
}
