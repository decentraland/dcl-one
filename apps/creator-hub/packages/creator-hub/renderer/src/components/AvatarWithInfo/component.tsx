import React from 'react';
import Address from '@ui/atoms/Address';
import AvatarFace, { type AvatarFaceProps } from '@ui/atoms/AvatarFace';
import Text from '@ui/atoms/Text';
import { useProfile } from '/@/hooks/useProfile';
import { Row } from '/@/components/Row';
import { CopyToClipboard } from '/@/components/CopyToClipboard';
import styles from './component.module.css';
import './styles.css';

type Props = {
  isLoading?: boolean;
  value: string;
  size?: AvatarFaceProps['size'];
};

export const AvatarWithInfo: React.FC<Props> = React.memo(
  ({ value, isLoading: externalIsLoading, size = 'small' }) => {
    const { avatar, isLoading: isFetchingProfile } = useProfile(value);
    const isLoading = !value || externalIsLoading || isFetchingProfile;

    if (isLoading) {
      return (
        <Row className="Avatar">
          <AvatarFace
            className={styles.face}
            size={size}
            inline
          />
          <span
            className={styles.skeleton}
            aria-hidden="true"
          />
        </Row>
      );
    }

    return (
      <Row className="Avatar">
        <AvatarFace
          className={styles.face}
          avatar={avatar}
          size={size}
          inline
        />
        <CopyToClipboard
          text={value}
          showPopup
        >
          <Text className="Paragraph">
            {avatar?.name && <span className="Name">{avatar.name}</span>}
            <Address
              value={value}
              shorten
            />
          </Text>
        </CopyToClipboard>
      </Row>
    );
  },
);
