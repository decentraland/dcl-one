import React from 'react';
import Text from '@ui/atoms/Text';

export type FooterProps = {
  version: string;
};

export const Footer: React.FC<FooterProps> = React.memo(({ version }) => (
  <footer className="Footer">
    <Text variant="body2">Decentraland Creator Hub {version}</Text>
  </footer>
));
