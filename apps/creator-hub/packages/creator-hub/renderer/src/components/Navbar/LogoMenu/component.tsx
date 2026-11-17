import { useCallback, useRef, useState, type MouseEvent } from 'react';
import { Menu, MenuItem } from '@ui/atoms/Menu';
import Tooltip from '@ui/atoms/Tooltip';
import { ArrowRightIcon } from '/@/components/Icons/material';

import { misc } from '#preload';
import logo from '/assets/images/logo-editor.png';
import { t } from '/@/modules/store/translation/utils';
import { CONTACT_SUPPORT_URL, DISCORD_URL, HELP_FAQ_URL } from '/@/modules/utils';

import { ReportIssue } from '../../Modals/ReportIssue';

import type { Props } from './types';

import './styles.css';

export function LogoMenu({ onClickAbout, onClickSettings, onClickCheckForUpdates }: Props) {
  const logoRef = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [helpAnchor, setHelpAnchor] = useState<HTMLElement | null>(null);
  const [reportOpen, setReportOpen] = useState(false);

  const close = useCallback(() => {
    setHelpAnchor(null);
    setOpen(false);
  }, []);

  const openHelp = useCallback((event: MouseEvent<HTMLElement>) => {
    setHelpAnchor(event.currentTarget);
  }, []);

  const handleClickAbout = useCallback(() => {
    close();
    onClickAbout();
  }, [close, onClickAbout]);

  const handleClickCheckForUpdates = useCallback(() => {
    close();
    onClickCheckForUpdates();
  }, [close, onClickCheckForUpdates]);

  const handleClickSettings = useCallback(() => {
    close();
    onClickSettings();
  }, [close, onClickSettings]);

  const handleClickReport = useCallback(() => {
    close();
    setReportOpen(true);
  }, [close]);

  const handleClickExternal = useCallback(
    (url: string) => () => {
      close();
      misc.openExternal(url);
    },
    [close],
  );

  return (
    <>
      <Tooltip
        label={t('navbar.logo_menu.main_menu_tooltip')}
        side="bottom"
        tipClassName="MainMenuTooltip"
        portal
        labelChild
      >
        <button
          type="button"
          className="LogoButton"
          ref={logoRef}
          data-testid="logo-menu-button"
          aria-haspopup="menu"
          aria-expanded={open ? 'true' : undefined}
          onClick={() => (open ? close() : setOpen(true))}
        >
          <img
            src={logo}
            alt="Decentraland Creator Hub"
          />
        </button>
      </Tooltip>
      <Menu
        className="LogoMenu"
        anchor={logoRef.current}
        open={open}
        onClose={close}
        placement="bottom-start"
        modal
      >
        <div className="MuiPaper-root">
          <MenuItem
            data-testid="logo-menu-about"
            onClick={handleClickAbout}
          >
            {t('navbar.logo_menu.about')}
          </MenuItem>
          <MenuItem
            data-testid="logo-menu-check-updates"
            onClick={handleClickCheckForUpdates}
          >
            {t('navbar.logo_menu.check_for_updates')}
          </MenuItem>
          <MenuItem
            data-testid="logo-menu-settings"
            onClick={handleClickSettings}
          >
            {t('navbar.logo_menu.settings')}
          </MenuItem>
          <MenuItem
            data-testid="logo-menu-report-bug"
            onClick={handleClickReport}
          >
            {t('navbar.logo_menu.report_bug')}
          </MenuItem>
          <MenuItem
            className="LogoMenuSubmenuItem"
            data-testid="logo-menu-help"
            aria-haspopup="menu"
            aria-expanded={helpAnchor ? 'true' : undefined}
            onClick={openHelp}
            onMouseEnter={openHelp}
          >
            {t('navbar.logo_menu.help.label')}
            <ArrowRightIcon fontSize="small" />
          </MenuItem>
        </div>
      </Menu>
      <Menu
        className="LogoMenu LogoSubmenu"
        anchor={helpAnchor}
        open={open && Boolean(helpAnchor)}
        onClose={() => setHelpAnchor(null)}
        placement="right-start"
        // The ui2 submenu was 4px under the top of its item.
        crossOffset={4}
      >
        <div className="MuiPaper-root">
          <MenuItem onClick={handleClickExternal(HELP_FAQ_URL)}>
            {t('navbar.logo_menu.help.faq')}
          </MenuItem>
          <MenuItem onClick={handleClickExternal(CONTACT_SUPPORT_URL)}>
            {t('navbar.logo_menu.help.contact_support')}
          </MenuItem>
          <MenuItem onClick={handleClickExternal(DISCORD_URL)}>
            {t('navbar.logo_menu.help.discord')}
          </MenuItem>
        </div>
      </Menu>
      <ReportIssue
        open={reportOpen}
        source="logo-menu"
        onClose={() => setReportOpen(false)}
      />
    </>
  );
}
