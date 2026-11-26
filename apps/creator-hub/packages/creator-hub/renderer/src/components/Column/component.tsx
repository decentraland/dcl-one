import { cx } from '/@/lib/cx';
import type { Props } from './types';

import './styles.css';

export function Column({ children, className }: Props) {
  return <div className={cx('Column', className)}>{children}</div>;
}
