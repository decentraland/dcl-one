import Button from '@ui/atoms/Button';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import logo from '/assets/images/logo-editor.png';
import { Modal } from '../index';

import type { Props } from './types';

import './styles.css';

export function About({ open, version, onClose }: Props) {
  return (
    <Modal
      size="tiny"
      open={open}
      title={t('modal.about.title')}
      onClose={onClose}
    >
      <div className="AboutModal">
        <img
          className="AboutModalLogo"
          src={logo}
          alt="Decentraland Creator Hub"
        />
        <Text
          variant="h6"
          className="AboutModalName"
        >
          {t('modal.about.app_name')}
        </Text>
        {version && (
          <Text
            variant="body2"
            className="AboutModalVersion"
          >
            {`v${version}`}
          </Text>
        )}
        <Button
          className="AboutModalAction"
          variant="primary"
          fullWidth
          onClick={onClose}
        >
          {t('modal.about.action')}
        </Button>
      </div>
    </Modal>
  );
}
