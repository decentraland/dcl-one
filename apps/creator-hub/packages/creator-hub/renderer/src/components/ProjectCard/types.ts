import type { ReactNode } from 'react';
import type { TextProps } from '@ui/atoms/Text';

import type { Status } from '/shared/types/async';

import type { Option } from '../Dropdown/types';

export type Props = {
  title: string;
  description?: string;
  imageUrl?: string;
  videoUrl?: string;
  content?: ReactNode;
  width?: number;
  height?: number;
  publishedAt?: number;
  dropdownOptions?: Option[];
  titleVariant?: TextProps['variant'];
  status?: Status;
  'data-testid'?: string;
  onClick: () => void;
};
