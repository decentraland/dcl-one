import { useState, useCallback, useEffect } from 'react';
import Button from '@ui/atoms/HubButton';
import Checkbox from '@ui/atoms/Checkbox';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import { useSettings } from '/@/hooks/useSettings';
import { Modal } from '../index';

import './styles.css';

interface Props {
  open: boolean;
  onClose: (continued?: boolean) => void;
}

export function WarningModal({ open, onClose }: Props) {
  const { settings, updateAppSettings } = useSettings();
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    if (open) {
      setIsLoading(false);
      setDontShowAgain(false);
    }
  }, [open]);

  const handleContinue = useCallback(() => {
    if (isLoading) return;
    setIsLoading(true);

    if (dontShowAgain) {
      updateAppSettings({
        ...settings,
        previewOptions: {
          ...settings.previewOptions,
          showWarnings: false,
        },
      });
    }
    onClose(true);
  }, [isLoading, dontShowAgain, settings, updateAppSettings, onClose]);

  return (
    <Modal
      size="tiny"
      open={open}
    >
      <div className="WarningModal">
        <Text variant="h5">{t('modal.warning.title')}</Text>
        <Text
          variant="body1"
          className="message"
        >
          {t('modal.warning.message')}
        </Text>
        <div className="CheckboxContainer">
          <Checkbox
            checked={dontShowAgain}
            onChange={setDontShowAgain}
          >
            {t('modal.warning.dont_show_again')}
          </Checkbox>
        </div>
        <div className="ButtonContainer">
          <Button
            variant="outline"
            className="actionButton"
            onClick={() => onClose()}
          >
            {t('modal.warning.cancel')}
          </Button>
          <Button
            variant="primary"
            className="actionButton"
            onClick={handleContinue}
            disabled={isLoading}
          >
            {t('modal.warning.continue')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
