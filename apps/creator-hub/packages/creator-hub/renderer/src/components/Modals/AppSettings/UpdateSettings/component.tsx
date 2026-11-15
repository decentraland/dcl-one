import { useCallback, useEffect, useMemo, useState } from 'react';
import Button from '@ui/atoms/HubButton';
import Text from '@ui/atoms/Text';
import { InfoOutlinedIcon as InfoOutlined } from '/@/components/Icons/material';
import { t } from '/@/modules/store/translation/utils';
import { useDispatch, useSelector } from '#store';
import { checkForUpdates, downloadUpdate, installUpdate } from '/@/modules/store/settings/slice';
import { Row } from '../../../Row';
import { Column } from '../../../Column';
import { MarkdownRenderer } from '../MarkdownRenderer';

import './styles.css';

interface UpdateButtonProps {
  action: () => void;
  text: string;
  disabled?: boolean;
  color: 'primary' | 'secondary' | 'inherit';
}

export const UpdateSettings: React.FC<{ className?: string; autoCheck?: boolean }> = ({
  className = '',
  autoCheck = false,
}) => {
  const dispatch = useDispatch();
  const {
    downloadingUpdate: { progress, finished, isDownloading },
    updateInfo,
    releaseNotes,
  } = useSelector(state => state.settings);

  const checkForUpdatesStatus = useSelector(state => state.settings.checkForUpdates.status);

  const [hasCheckedForUpdates, setHasCheckedForUpdates] = useState(false);

  const shouldShowUpdateAvailable = useMemo(() => {
    return (
      hasCheckedForUpdates &&
      checkForUpdatesStatus === 'succeeded' &&
      updateInfo.available &&
      updateInfo.version &&
      !isDownloading
    );
  }, [
    hasCheckedForUpdates,
    checkForUpdatesStatus,
    updateInfo.available,
    updateInfo.version,
    isDownloading,
  ]);

  const handleCheckForUpdates = useCallback(() => {
    setHasCheckedForUpdates(true);
    dispatch(checkForUpdates({ autoDownload: false }));
  }, [dispatch]);

  // Entering from the logo menu's "Check for Updates" runs the check immediately,
  // so the user lands on this tab with the result already on its way.
  useEffect(() => {
    if (autoCheck) handleCheckForUpdates();
  }, [autoCheck, handleCheckForUpdates]);

  const handleInstallUpdate = useCallback(async () => {
    dispatch(installUpdate());
  }, []);

  const handleDownloadUpdate = useCallback(async () => {
    dispatch(downloadUpdate());
  }, []);

  const getButtonProps = useCallback((): UpdateButtonProps => {
    if (checkForUpdatesStatus === 'loading') {
      return {
        action: () => {},
        text: t('editor.loading.title'),
        disabled: true,
        color: 'secondary',
      };
    }

    if (progress > 0 && !finished) {
      return {
        action: () => {},
        text: t('modal.app_settings.update.downloading', { progress }),
        disabled: true,
        color: 'inherit',
      };
    }

    if (!hasCheckedForUpdates || checkForUpdatesStatus === 'idle') {
      return {
        action: handleCheckForUpdates,
        text: t('modal.app_settings.update.check'),
        color: 'secondary',
      };
    }

    if (updateInfo.available) {
      const buttonText = updateInfo.isDownloaded
        ? t('modal.app_settings.update.install')
        : t('modal.app_settings.update.update');

      return {
        action: updateInfo.isDownloaded ? handleInstallUpdate : handleDownloadUpdate,
        text: buttonText,
        color: 'primary',
      };
    }

    return {
      action: handleCheckForUpdates,
      text: t('modal.app_settings.update.check'),
      color: 'secondary',
    };
  }, [
    checkForUpdatesStatus,
    updateInfo,
    progress,
    finished,
    hasCheckedForUpdates,
    isDownloading,
    handleCheckForUpdates,
    handleDownloadUpdate,
    handleInstallUpdate,
  ]);

  const canInstallNewVersion = useMemo(
    () => updateInfo.available && updateInfo.isDownloaded && hasCheckedForUpdates,
    [updateInfo.available, updateInfo.isDownloaded, hasCheckedForUpdates],
  );

  const buttonProps = getButtonProps();

  const renderStatusMessage = () => {
    if (shouldShowUpdateAvailable && releaseNotes) {
      return (
        <Text
          variant="h6"
          className="update-settings__release-notes-header"
        >
          {t('modal.app_settings.update.update_available')}: v{releaseNotes.version}
        </Text>
      );
    }

    if (progress > 0 && !finished) {
      return (
        <div className="update-settings__progress-container">
          <Text variant="body2">{t('modal.app_settings.update.applying')}</Text>
          <Text variant="body2">{t('modal.app_settings.update.dont_close')}</Text>
        </div>
      );
    }

    if (
      hasCheckedForUpdates &&
      updateInfo.available === false &&
      checkForUpdatesStatus === 'succeeded'
    ) {
      return (
        <Text
          variant="subtitle1"
          className="update-settings__up-to-date-message"
        >
          {t('modal.app_settings.version.up_to_date')}
        </Text>
      );
    }

    return null;
  };

  const renderReleaseNotes = () => {
    if (!shouldShowUpdateAvailable || !releaseNotes || !releaseNotes.content.length) {
      return null;
    }

    return (
      <div className="update-settings__release-notes">
        <div className="update-settings__release-notes-content">
          {releaseNotes.content.length > 0 && (
            <div className="update-settings__release-notes-section">
              <MarkdownRenderer content={releaseNotes.content} />
            </div>
          )}
        </div>
      </div>
    );
  };

  return (
    <Column className={`update-settings ${className}`}>
      <div className="update-settings__status-area">
        {renderStatusMessage()}
        {renderReleaseNotes()}
      </div>
      <div className="update-settings__button-area">
        <Button
          variant="primary"
          tone={buttonProps.color === 'primary' ? undefined : 'neutral'}
          data-color={buttonProps.color}
          onClick={buttonProps.action}
          disabled={buttonProps.disabled}
          fullWidth
        >
          {buttonProps.text}
        </Button>
        {canInstallNewVersion && (
          <Row className="update-settings__message-container">
            <InfoOutlined />
            <Text variant="body2">{t('modal.app_settings.update.auto_restart')}</Text>
          </Row>
        )}
      </div>
    </Column>
  );
};
