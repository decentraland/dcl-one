import React, { useCallback, useState, type MouseEvent } from 'react';
import { cx } from '/@/lib/cx';
import IconButton from '@ui/atoms/IconButton';
import { Menu, MenuItem, MenuSeparator } from '@ui/atoms/Menu';

import { KebabIcon } from '../Icons';
import type { Option, Props } from './types';
import { useArrowKeyed } from './useArrowKeyed';
import styles from './component.module.css';

const MENU_GAP = 9;

function Dropdown(props: Props) {
  const { label, options, className, selected, variant = 'ghost' } = props;
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const { arrowKeyedClass, onKeyDownCapture, resetArrowKeyed } = useArrowKeyed();
  const open = Boolean(anchorEl);
  const hasGlyphs = options.some(option => option.icon);

  const handleClick = useCallback((e: MouseEvent<HTMLElement>) => {
    e.stopPropagation();
    e.currentTarget.focus();
    resetArrowKeyed();
    setAnchorEl(e.currentTarget);
  }, []);

  const handleSelect = useCallback((e: MouseEvent<HTMLElement>, option: Option) => {
    e.stopPropagation();
    setAnchorEl(null);
    option.handler();
  }, []);

  const handleClose = useCallback(() => setAnchorEl(null), []);

  return (
    <>
      <IconButton
        className={styles.trigger}
        data-look={variant}
        onClick={handleClick}
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <KebabIcon />
      </IconButton>
      <Menu
        className={cx(styles.menu, className, arrowKeyedClass)}
        anchor={anchorEl}
        open={open}
        onClose={handleClose}
        placement="bottom-end"
        offset={MENU_GAP}
        aria-label={label}
      >
        {options.flatMap(option => [
          <MenuItem
            key={option.text}
            className={styles.item}
            tone={option.tone === 'danger' ? 'danger' : 'default'}
            selected={option.text === selected}
            onClick={e => handleSelect(e, option)}
            onKeyDownCapture={onKeyDownCapture}
            disabled={option.disabled}
          >
            {hasGlyphs && <span className="MenuGlyph">{option.icon}</span>}
            {option.text}
          </MenuItem>,
          ...(option.divider
            ? [
                <MenuSeparator
                  key={`${option.text}-separator`}
                  className={styles.separator}
                />,
              ]
            : []),
        ])}
      </Menu>
    </>
  );
}

export const DropdownMemo = React.memo(Dropdown);
