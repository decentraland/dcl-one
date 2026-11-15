import { cx } from '/@/lib/cx';
import ManaMark from '@ui/atoms/ManaMark';

import './styles.css';

type Props = {
  /** Already formatted, so callers decide about decimals and "no data". */
  children: string;
  className?: string;
};

/** A MANA amount with its icon. Read-only: it has no button and takes no focus. */
export function ManaValue({ children, className }: Props) {
  return (
    <div className={cx('ManaValue', className)}>
      <ManaMark
        network="polygon"
        size={20}
      />
      {children}
    </div>
  );
}
