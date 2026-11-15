import { useCallback, useState, useEffect, useRef } from 'react';
import Button from '@ui/atoms/HubButton';
import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/HubSpinner';
import Text from '@ui/atoms/Text';
import { SignalWifiOffIcon, RefreshIcon, CloseIcon } from '/@/components/Icons/material';

import { t } from '/@/modules/store/translation/utils';
import { useConnectionStatus } from '/@/hooks/useConnectionStatus';
import { ConnectionStatus } from '/@/lib/connection';
import { Modal } from '../Modals';
import './modal-styles.css';

const COOLDOWN_MS = 3_000;

interface ConnectionModalProps {
  open: boolean;
  onClose: () => void;
}

export function ConnectionModal({ open, onClose }: ConnectionModalProps) {
  const { checkNow, status } = useConnectionStatus();
  const [isChecking, setIsChecking] = useState(false);
  const [cooldown, setCooldown] = useState(0);
  const cooldownIntervalRef = useRef<NodeJS.Timeout>();

  const handleRetry = useCallback(async () => {
    if (isChecking || cooldown > 0) return;

    setIsChecking(true);
    try {
      await checkNow();
    } finally {
      setIsChecking(false);

      setCooldown(COOLDOWN_MS / 1000);
      cooldownIntervalRef.current = setInterval(() => {
        setCooldown(prev => {
          if (prev <= 1) {
            if (cooldownIntervalRef.current) {
              clearInterval(cooldownIntervalRef.current);
            }
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
    }
  }, [checkNow, isChecking, cooldown]);

  useEffect(() => {
    return () => {
      if (cooldownIntervalRef.current) {
        clearInterval(cooldownIntervalRef.current);
      }
    };
  }, []);

  // auto-close when connection is restored
  useEffect(() => {
    if (status === ConnectionStatus.ONLINE && open) {
      onClose();
    }
  }, [status, open, onClose]);

  const isDisabled = isChecking || cooldown > 0;

  return (
    <Modal
      size="tiny"
      open={open}
    >
      <div className="ConnectionModal">
        <div className="CloseButtonContainer">
          <IconButton
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseIcon fontSize="medium" />
          </IconButton>
        </div>
        <div className="icon-container">
          <SignalWifiOffIcon className="icon offline" />
        </div>
        <Text
          variant="h5"
          className="title"
        >
          {t('connection.offline.title')}
        </Text>
        <Text
          variant="body1"
          className="message"
        >
          {t('connection.offline.message')}
        </Text>
        <div className="actions">
          <Button
            variant="primary"
            size="lg"
            onClick={handleRetry}
            disabled={isDisabled}
            className="retry-button"
            startIcon={
              isChecking ? (
                <Spinner
                  ring="wide"
                  role="progressbar"
                  size={20}
                  color="primary"
                />
              ) : (
                <RefreshIcon />
              )
            }
          >
            {isChecking
              ? t('connection.offline.checking')
              : cooldown > 0
                ? `${t('connection.offline.retry')} (${cooldown}s)`
                : t('connection.offline.retry')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
