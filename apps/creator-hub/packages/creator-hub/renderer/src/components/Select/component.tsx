import {
  Children,
  isValidElement,
  type ElementType,
  type ReactElement,
  type ReactNode,
} from 'react';
import { cx } from '/@/lib/cx';
import { Select as Ui3Select, SelectOption } from '@ui/atoms/Select';

import './styles.css';

/** What `onChange` gets: the new value is in `target.value`. */
export type SelectChangeEvent<T> = Event & { target: { value: T; name: string } };

export type Props<T extends string | string[]> = {
  value?: T | '';
  onChange?: (event: SelectChangeEvent<T>, child?: ReactNode) => void;
  /** Each element that has a `value` prop is one option. */
  children?: ReactNode;
  className?: string;
  /** Accepted for the callers. The look is always the one without a border. */
  variant?: 'standard' | 'outlined' | 'filled';
  multiple?: boolean;
  maxSelected?: number;
  /** The id of the element that names the field. */
  labelId?: string;
  id?: string;
  name?: string;
  disabled?: boolean;
  error?: boolean;
  IconComponent?: ElementType<{ className?: string }>;
  renderValue?: (value: T) => ReactNode;
  onOpen?: () => void;
  onClose?: () => void;
  MenuProps?: { className?: string };
  'aria-label'?: string;
};

type OptionProps = {
  value: string | number;
  disabled?: boolean;
  className?: string;
  children?: ReactNode;
};

const isOption = (node: unknown): node is ReactElement<OptionProps> =>
  isValidElement<Partial<OptionProps>>(node) && node.props.value !== undefined;

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

export function Select<T extends string | string[]>({
  value,
  onChange,
  children,
  className,
  variant: _variant,
  multiple,
  maxSelected,
  labelId,
  name,
  IconComponent,
  renderValue,
  onOpen,
  onClose,
  MenuProps,
  ...props
}: Props<T>) {
  const nodes = Children.toArray(children);

  const handleChange = (next: string | string[]) => {
    if (!onChange) return;
    const event = new Event('change', { bubbles: true });
    Object.defineProperty(event, 'target', { value: { value: next, name: name ?? '' } });
    const child = nodes.find(node => isOption(node) && String(node.props.value) === next);
    onChange(event as SelectChangeEvent<T>, child);
  };

  const shared = {
    ...props,
    name,
    variant: 'plain',
    placement: 'bottom',
    className: cx('Select', className),
    menuClassName: cx('SelectMenu', MenuProps?.className),
    'aria-labelledby': labelId,
    icon: IconComponent ? <IconComponent className="SelectIcon" /> : arrow,
    onOpenChange: (open: boolean) => (open ? onOpen : onClose)?.(),
    children: nodes.map(node =>
      isOption(node) ? (
        <SelectOption
          key={node.key}
          value={String(node.props.value)}
          disabled={node.props.disabled}
          className={node.props.className}
        >
          {node.props.children}
        </SelectOption>
      ) : (
        node
      ),
    ),
  } as const;

  if (multiple) {
    return (
      <Ui3Select
        {...shared}
        multiple
        value={Array.isArray(value) ? value : []}
        maxSelected={maxSelected}
        renderValue={renderValue && (selected => renderValue(selected as T))}
        onChange={handleChange}
      />
    );
  }

  return (
    <Ui3Select
      {...shared}
      value={typeof value === 'string' ? value : ''}
      renderValue={renderValue && (selected => renderValue(selected as T))}
      onChange={handleChange}
    />
  );
}
