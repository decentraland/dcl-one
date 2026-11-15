import Button from '@ui/atoms/HubButton';
import IconButton from '@ui/atoms/IconButton';
import { CloseIcon, InfoOutlinedIcon } from '@ui/atoms/icons-material';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import InfluencePng from '/assets/images/influence.png';

import { actions } from '/@/modules/store/settings';
import { useDispatch } from '#store';
import { Modal } from '../index';
import { Row } from '../../Row';
import './styles.css';

interface Props {
  open: boolean;
  onClose: () => void;
  version: string;
}

export function UpdateAvailableModal({ open, onClose, version }: Props) {
  const dispatch = useDispatch();
  return (
    <Modal
      size="tiny"
      open={open}
    >
      <div className="UpdateAvailableModal">
        <div className="CloseButtonContainer">
          <IconButton
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseIcon fontSize="medium" />
          </IconButton>
        </div>
        <Text variant="h5">{t('modal.app_settings.update.update_available')}</Text>
        <Text
          variant="body1"
          className="version"
        >
          {t('modal.app_settings.version.label', { version })}
        </Text>
        <div className="ImageButtonContainer">
          <img
            src={InfluencePng}
            alt="Update available"
          />
          <div className="ButtonContainer">
            <Button
              variant="primary"
              className="actionButton"
              onClick={() => {
                dispatch(actions.installUpdate());
              }}
            >
              {t('modal.app_settings.update.install')}
            </Button>
            <Row className="update-settings__message-container">
              <InfoOutlinedIcon />
              <Text variant="body2">{t('modal.app_settings.update.auto_restart')}</Text>
            </Row>
          </div>
        </div>
      </div>
    </Modal>
  );
}
