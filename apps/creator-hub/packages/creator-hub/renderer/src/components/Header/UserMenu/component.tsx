import { useCallback, useState } from 'react';
import AvatarFace from '@ui/atoms/AvatarFace';
import Button from '@ui/atoms/Button';
import { Menu, MenuItem } from '@ui/atoms/Menu';
import Tooltip from '@ui/atoms/Tooltip';
import { t } from '/@/modules/store/translation/utils';
import type { Props } from './types';

import './styles.css';

export function UserMenu({ avatar, isSignedIn, onClickSignOut, onClickSignIn }: Props) {
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const open = Boolean(anchorEl);
  const handleClick = useCallback((event: React.MouseEvent<HTMLElement, MouseEvent>) => {
    const target = event.currentTarget;
    setAnchorEl(current => (current ? null : target));
  }, []);

  const handleClose = useCallback(() => {
    setAnchorEl(null);
  }, []);

  return isSignedIn ? (
    <>
      <Tooltip
        label={t('navbar.user_menu.account_tooltip')}
        side="bottom"
        tipClassName="AccountTooltip"
        portal
        labelChild
      >
        <button
          type="button"
          id="AvatarButton"
          className="AvatarButton"
          data-testid="user-menu-avatar-button"
          aria-controls={open ? 'UserMenu' : undefined}
          aria-haspopup="true"
          aria-expanded={open ? 'true' : undefined}
          onClick={handleClick}
        >
          <AvatarFace
            size="medium"
            avatar={avatar}
          />
        </button>
      </Tooltip>
      <Menu
        id="UserMenu"
        className="UserMenu"
        anchor={anchorEl}
        open={open}
        onClose={handleClose}
        offset={0}
        modal
        aria-labelledby="AvatarButton"
      >
        <MenuItem
          data-testid="user-menu-sign-out"
          onClick={onClickSignOut}
        >
          {t('navbar.user_menu.sign_out')}
        </MenuItem>
      </Menu>
    </>
  ) : (
    <Button
      className="SignInButton"
      data-testid="user-menu-sign-in"
      onClick={onClickSignIn}
      variant="primary"
    >
      {t('navbar.user_menu.sign_in')}
    </Button>
  );
}
