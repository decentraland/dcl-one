import { cx } from '/@/lib/cx';
import { Select, type SelectProps } from '@ui/atoms/Select';

import './styles.css';

const arrow = (
  <svg
    className="SelectArrow"
    viewBox="0 0 24 24"
    focusable="false"
    aria-hidden="true"
  >
    <path d="M7 10l5 5 5-5z" />
  </svg>
);

/**
 * The borderless select of the list toolbars (sort, view, time window): a fill on hover and
 * focus, a filled arrow and its list right under it.
 */
export function ToolbarSelect<V extends string>(props: SelectProps<V>) {
  return (
    <Select
      placement="bottom"
      icon={arrow}
      {...props}
      variant="plain"
      className={cx('Select', props.className)}
      menuClassName={cx('SelectMenu', props.menuClassName)}
    />
  );
}
