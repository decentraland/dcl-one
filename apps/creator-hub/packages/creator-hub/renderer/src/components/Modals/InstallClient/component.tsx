import { useCallback, useId } from 'react';
import { AppleIcon } from '@ui/atoms/BrandIcons';
import Text from '@ui/atoms/Text';
import { actions } from '/@/modules/store/editor';
import { useDispatch } from '#store';
import { t } from '/@/modules/store/translation/utils';
import { Modal } from '..';
import Button from '@ui/atoms/Button';
import LogoDCLSVG from '/assets/images/logo-dcl.svg';
import type { Props } from './types';

import './styles.css';

const MACOS = 'macOS';

function isMacOS() {
  const { userAgentData } = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = userAgentData?.platform;
  return platform ? platform === MACOS : navigator.userAgent.includes('Macintosh');
}

const windowsIcon = (
  <svg
    viewBox="0 0 24 24"
    width={20}
    height={20}
    aria-hidden="true"
    focusable="false"
  >
    <path
      fill="#fff"
      d="M3,12V6.75L9,5.43V11.91L3,12M20,3V11.75L10,11.9V5.21L20,3M3,13L9,13.09V19.9L3,18.75V13M20,13.25V22L10,20.09V13.1L20,13.25Z"
    />
  </svg>
);

export function InstallClient({ open, onClose }: Props) {
  const dispatch = useDispatch();
  const titleId = useId();

  const handleDownload = useCallback(() => {
    dispatch(actions.openExternalURL('https://decentraland.org/download/'));
    onClose();
  }, [dispatch, onClose]);
  const { icon, text } = isMacOS()
    ? {
        icon: (
          <AppleIcon
            size={14}
            className="AppleIcon"
          />
        ),
        text: t('modal.install_client.download_macos'),
      }
    : { icon: windowsIcon, text: t('modal.install_client.download_win') };

  return (
    <Modal
      className="InstallClientModal"
      aria-labelledby={titleId}
      open={open}
      size="tiny"
    >
      <div className="InstallClientBox">
        <div className="LogoContainer">
          <img
            src={LogoDCLSVG}
            alt="Decentraland Logo"
            className="Logo"
          />
        </div>
        <Text
          variant="h5"
          id={titleId}
          className="Title"
        >
          {t('modal.install_client.title')}
        </Text>
        <div className="Actions">
          <Button
            className="hub-button hub-button--primary"
            onClick={handleDownload}
            startIcon={icon}
          >
            {text}
          </Button>
          <Button
            variant="outline"
            className="hub-button hub-button--secondary hub-button--outlined"
            fullWidth
            onClick={onClose}
          >
            {t('modal.install_client.cancel')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
