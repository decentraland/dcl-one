import React, { useCallback, type InputHTMLAttributes, type ReactNode } from 'react';
import { cx } from '/@/lib/cx';
import TextField from '@ui/atoms/TextField';
import { SearchIcon } from '@ui/atoms/icons-material';
import { debounce } from '/shared/utils';
import styles from './component.module.css';
import './styles.css';

type Props = Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'size'> & {
  onChange?: (value: string) => void;
  /** Attributes for the input itself. They win over the same attribute on the component. */
  inputProps?: Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'size'>;
  startAdornment?: ReactNode;
  endAdornment?: ReactNode;
  error?: boolean;
  fullWidth?: boolean;
};

const Search: React.FC<Props> = React.memo(({ onChange, className, inputProps, ...props }) => {
  const onChangeDebounced = useCallback(
    debounce((event: React.ChangeEvent<HTMLInputElement>) => {
      if (onChange) {
        onChange(event.target.value);
      }
    }, 500),
    [onChange],
  );

  return (
    <TextField
      endAdornment={<SearchIcon />}
      {...props}
      {...inputProps}
      onChange={onChangeDebounced}
      className={cx('SearchInput', styles.search, className)}
    />
  );
});

export { Search };
