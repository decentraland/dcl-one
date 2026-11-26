import { type HTMLAttributes, memo } from 'react';
import { VscTrash as RemoveIcon } from 'react-icons/vsc';
import { cx } from '../../../lib/utils/cx';
import { Button } from '../../Button';
import './RemoveButton.css';

interface RemoveButtonProps extends Omit<HTMLAttributes<HTMLButtonElement>, 'color'> {
  variant?: 'default' | 'add';
}

const RemoveButton = memo<RemoveButtonProps>(props => {
  const { children, variant = 'default', className, ...rest } = props;

  if (variant === 'add') {
    return (
      <button
        className={cx('RemoveButton', 'RemoveButton--add', className)}
        {...rest}
      >
        <RemoveIcon /> {children}
      </button>
    );
  }

  return (
    <Button
      variant="secondary"
      size="sm"
      className={cx('RemoveButton', className)}
      {...rest}
    >
      <RemoveIcon /> {children}
    </Button>
  );
});

export default RemoveButton;
