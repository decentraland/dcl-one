import React from 'react';
import Address from '@ui/atoms/Address';
import AvatarFace, { type AvatarFaceProps } from '@ui/atoms/AvatarFace';
import Text from '@ui/atoms/Text';
import { useProfile } from '/@/hooks/useProfile';
import { Row } from '/@/components/Row';
import { CopyToClipboard } from '/@/components/CopyToClipboard';
import styles from './component.module.css';
import './styles.css';

export type Props = {
  isLoading?: boolean;
  value: string;
  size?: AvatarFaceProps['size'];
  icon?: React.ReactNode;
  name?: string;
  subtitle?: string;
};

export const AvatarWithInfo: React.FC<Props> = React.memo(
  ({ value, isLoading: externalIsLoading, size = 'small', icon, name, subtitle }) => {
    const skipProfile = !!icon;
    const { avatar, isLoading: isFetchingProfile } = useProfile(skipProfile ? '' : value);
    const isLoading = !skipProfile && (!value || externalIsLoading || isFetchingProfile);

    if (isLoading) {
      return (
        <Row className="Avatar">
          <AvatarFace
            className="MuiAvatar-root"
            size={size}
            inline
          />
          <span
            className={`MuiSkeleton-root ${styles.skeleton}`}
            aria-hidden="true"
          />
        </Row>
      );
    }

    if (icon) {
      return (
        <Row className="Avatar">
          {icon}
          <Text className="Paragraph">
            {name && <span className="Name">{name}</span>}
            {subtitle && <span>{subtitle}</span>}
          </Text>
        </Row>
      );
    }

    return (
      <Row className="Avatar">
        <AvatarFace
          className="MuiAvatar-root"
          avatar={avatar}
          size={size}
          inline
        />
        <CopyToClipboard
          text={value}
          showPopup
        >
          <Text className="Paragraph">
            {(name ?? avatar?.name) && <span className="Name">{name ?? avatar?.name}</span>}
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
