import { useCallback, useState } from 'react';
import { Link } from 'react-router';
import { cx } from '/@/lib/cx';
import { useDispatch, useSelector } from '#store';
import { useFeatureFlags } from '/@/hooks/useFeatureFlags';
import { FeatureFlag } from '/@/modules/store/featureFlags';
import { selectors as creatorFundSelectors } from '/@/modules/store/creatorFund';
import type { AppState } from '#store';
import { useAuth } from '/@/hooks/useAuth';
import { useEditor } from '/@/hooks/useEditor';
import { t } from '/@/modules/store/translation/utils';
import { actions } from '/@/modules/store/settings';
import { AppSettings, SettingsTab } from '../Modals/AppSettings';
import { About } from '../Modals/About';
import { CreateButton } from '../CreateButton';
import { Header } from '../Header';
import { NotificationDot } from '../NotificationDot';
import { LogoMenu } from './LogoMenu';
import './styles.css';

export enum NavbarItem {
  HOME = 'home',
  SCENES = 'scenes',
  TEMPLATES = 'templates',
  COLLECTIONS = 'collections',
  ANALYTICS = 'analytics',
  LEARN = 'learn',
  MANAGE = 'manage',
  EARN = 'earn',
  MORE = 'more',
}

function MenuItem(props: {
  item: NavbarItem;
  active: NavbarItem;
  disable?: boolean;
  label?: string;
  children?: React.ReactNode;
  // Declared and forwarded explicitly: this component does not spread its props, so a
  // `data-testid` passed by a caller would otherwise be dropped without a type error —
  // TypeScript exempts hyphenated JSX attributes from excess-property checks.
  'data-testid'?: string;
}) {
  const isActive = props.active === props.item;
  return !props.disable ? (
    <Link
      to={`/${props.item}`}
      className={cx('menu-item', { active: isActive })}
      aria-current={isActive ? 'page' : undefined}
      aria-label={props.label}
      data-testid={props['data-testid']}
    >
      <span className="label">
        {t(`navbar.menu.${props.item}`)}
        {props.children}
      </span>
    </Link>
  ) : null;
}

export function Navbar(props: { active: NavbarItem }) {
  const openAppSettings = useSelector((state: AppState) => state.settings.openAppSettingsModal);
  const { isEnabled } = useFeatureFlags();
  const { isSignedIn } = useAuth();
  const enrollment = useSelector((state: AppState) => state.creatorFund.enrollment);
  const payoutSetupPending = useSelector(creatorFundSelectors.isPayoutSetupPending);
  const notJoined = enrollment.status === 'succeeded' && enrollment.data === null;
  let earnLabel: string | undefined;
  if (notJoined) earnLabel = t('navbar.earn_not_joined');
  else if (payoutSetupPending) earnLabel = t('navbar.earn_payout_pending');
  const showEarnDot =
    isSignedIn &&
    isEnabled(FeatureFlag.CREATOR_FUND) &&
    props.active !== NavbarItem.EARN &&
    !!earnLabel;
  const dispatch = useDispatch();
  const { version } = useEditor();
  const [openAbout, setOpenAbout] = useState(false);
  const [settingsTab, setSettingsTab] = useState(SettingsTab.SCENES);
  const [autoCheckForUpdates, setAutoCheckForUpdates] = useState(false);

  const handleOpenAbout = useCallback(() => setOpenAbout(true), []);
  const handleCloseAbout = useCallback(() => setOpenAbout(false), []);

  const handleOpenSettings = useCallback(() => {
    setSettingsTab(SettingsTab.SCENES);
    setAutoCheckForUpdates(false);
    dispatch(actions.setOpenAppSettingsModal(true));
  }, [dispatch]);

  // "Check for Updates" has no surface of its own — it opens App Settings on the
  // About tab and kicks off the check there.
  const handleCheckForUpdates = useCallback(() => {
    setSettingsTab(SettingsTab.ABOUT);
    setAutoCheckForUpdates(true);
    dispatch(actions.setOpenAppSettingsModal(true));
  }, [dispatch]);

  const handleCloseSettings = useCallback(() => {
    setAutoCheckForUpdates(false);
    dispatch(actions.setOpenAppSettingsModal(false));
  }, [dispatch]);

  return (
    <Header classNames={cx('Navbar')}>
      <>
        <div className="logo">
          <LogoMenu
            onClickAbout={handleOpenAbout}
            onClickSettings={handleOpenSettings}
            onClickCheckForUpdates={handleCheckForUpdates}
          />
        </div>
        <div className="menu">
          <MenuItem
            data-testid="navbar-menu-home"
            item={NavbarItem.HOME}
            active={props.active}
          />
          <MenuItem
            data-testid="navbar-menu-scenes"
            item={NavbarItem.SCENES}
            active={props.active}
          />
          <MenuItem
            data-testid="navbar-menu-templates"
            item={NavbarItem.TEMPLATES}
            active={props.active}
          />
          <MenuItem
            data-testid="navbar-menu-collections"
            item={NavbarItem.COLLECTIONS}
            active={props.active}
            disable={true}
          />
          <MenuItem
            item={NavbarItem.ANALYTICS}
            active={props.active}
            disable={!isEnabled(FeatureFlag.ANALYTICS)}
          />
          <MenuItem
            data-testid="navbar-menu-manage"
            item={NavbarItem.MANAGE}
            active={props.active}
          />
          <MenuItem
            item={NavbarItem.EARN}
            active={props.active}
            disable={!isEnabled(FeatureFlag.CREATOR_FUND)}
            label={showEarnDot ? earnLabel : undefined}
          >
            {showEarnDot && <NotificationDot />}
          </MenuItem>
          <MenuItem
            data-testid="navbar-menu-learn"
            item={NavbarItem.LEARN}
            active={props.active}
          />
          <MenuItem
            data-testid="navbar-menu-more"
            item={NavbarItem.MORE}
            active={props.active}
          />
        </div>
      </>
      <>
        <div className="actions">
          <CreateButton />
        </div>
        <AppSettings
          open={openAppSettings}
          initialTab={settingsTab}
          autoCheckForUpdates={autoCheckForUpdates}
          onClose={handleCloseSettings}
        />
        <About
          open={openAbout}
          version={version}
          onClose={handleCloseAbout}
        />
      </>
    </Header>
  );
}
