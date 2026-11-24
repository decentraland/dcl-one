import React from 'react';
import { cx } from '../../../lib/utils/cx';

import type { Props } from './types';

import './Tabs.css';

const Tabs: React.FC<React.PropsWithChildren<Props>> = props => {
  const classNames = cx('Tabs', props.className);
  return <div className={classNames}>{props.children}</div>;
};

export default React.memo(Tabs);
