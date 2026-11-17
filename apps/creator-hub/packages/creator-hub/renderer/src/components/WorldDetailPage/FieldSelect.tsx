import { cx } from '/@/lib/cx';
import { CheckIcon } from '@ui/atoms/icons-material';
import { Select, type SelectProps } from '@ui/atoms/Select';

import { ChevronDownIcon } from '../Icons';

import styles from './FieldSelect.module.css';

export { SelectOption } from '@ui/atoms/Select';

/**
 * The outlined select of the World detail tabs, with the dark menu surface. The open list is modal, like the
 * menu it replaces: a press outside only closes it, and the page behind does not scroll.
 */
export function FieldSelect<V extends string = string>(props: SelectProps<V>) {
  return (
    <Select
      modal
      lockScroll
      {...props}
      icon={props.icon ?? <ChevronDownIcon />}
      className={cx(styles.field, props.className)}
      menuClassName={cx(styles.menu, props.menuClassName)}
    />
  );
}

/** The 16px slot at the start of an option that holds the tick of a chosen option. */
export function OptionCheck({ checked }: { checked: boolean }) {
  return <span className={styles.check}>{checked && <CheckIcon aria-hidden />}</span>;
}
