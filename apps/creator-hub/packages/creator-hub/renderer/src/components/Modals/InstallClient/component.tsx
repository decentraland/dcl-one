import { useCallback } from 'react';
import {
  DownloadButtonAppleIcon,
  DownloadButtonWindowsIcon,
} from 'decentraland-ui2/dist/components/DownloadButton/DownloadButton.styled';
import { Box, OperativeSystem, Typography } from 'decentraland-ui2';
import { actions } from '/@/modules/store/editor';
import { useDispatch } from '#store';
import { t } from '/@/modules/store/translation/utils';
import { Modal } from '..';
import { Button } from '../../Button';
import LogoDCLSVG from '/assets/images/logo-dcl.svg';
import type { Props } from './types';

import './styles.css';

function isMacOS() {
  const { userAgentData } = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = userAgentData?.platform;
  return platform ? platform === OperativeSystem.MACOS : navigator.userAgent.includes('Macintosh');
}

export function InstallClient({ open, onClose }: Props) {
  const dispatch = useDispatch();

  const handleDownload = useCallback(() => {
    dispatch(actions.openExternalURL('https://decentraland.org/download/'));
    onClose();
  }, [dispatch, onClose]);
  const { icon, text } = isMacOS()
    ? { icon: <DownloadButtonAppleIcon />, text: t('modal.install_client.download_macos') }
    : { icon: <DownloadButtonWindowsIcon />, text: t('modal.install_client.download_win') };

  return (
    <Modal
      className="InstallClientModal"
      data-testid="install-client-modal"
      open={open}
      size="tiny"
    >
      <Box className="InstallClientBox">
        <Box className="LogoContainer">
          <img
            src={LogoDCLSVG}
            alt="Decentraland Logo"
            className="Logo"
          />
        </Box>
        <Typography
          variant="h5"
          className="Title"
        >
          {t('modal.install_client.title')}
        </Typography>
        <Box className="Actions">
          <Button
            variant="contained"
            color="primary"
            onClick={handleDownload}
            startIcon={icon}
          >
            {text}
          </Button>
          <Button
            variant="outlined"
            color="secondary"
            fullWidth
            onClick={onClose}
          >
            {t('modal.install_client.cancel')}
          </Button>
        </Box>
      </Box>
    </Modal>
  );
}
