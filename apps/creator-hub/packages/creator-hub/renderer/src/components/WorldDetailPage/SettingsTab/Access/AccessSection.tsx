import { useEffect, useRef, useState } from 'react';
import { cx } from '/@/lib/cx';

import { useDispatch, useSelector } from '#store';
import { WorldPermissionType } from '/@/lib/worlds';
import { fetchWorldPermissions } from '/@/modules/store/management/slice';
import type { TranslationPath } from '/@/modules/store/translation/types';
import { t } from '/@/modules/store/translation/utils';

import Button from '@ui/atoms/Button';
import { FieldTitle } from '../../../LandDetailPage/DetailsTab/DetailsTab.styled';
import { DefaultModal } from '../../../Modals/DefaultModal';
import { FieldSelect, OptionCheck, SelectOption } from '../../FieldSelect';
import { InvitedList } from './InvitedList';
import { InviteForm } from './InviteForm';
import { PasswordModal } from './PasswordModal';
import { useAccessActions } from './useAccessActions';

import styles from './Access.module.css';

const TYPES: Record<
  | WorldPermissionType.Unrestricted
  | WorldPermissionType.AllowList
  | WorldPermissionType.SharedSecret,
  { label: TranslationPath; description: TranslationPath }
> = {
  [WorldPermissionType.Unrestricted]: {
    label: 'world_detail.settings.access.public',
    description: 'world_detail.settings.access.public_description',
  },
  [WorldPermissionType.AllowList]: {
    label: 'world_detail.settings.access.invitation_only',
    description: 'world_detail.settings.access.invitation_only_description',
  },
  [WorldPermissionType.SharedSecret]: {
    label: 'world_detail.settings.access.password_protected',
    description: 'world_detail.settings.access.password_protected_description',
  },
};

type AccessType = keyof typeof TYPES;

const typeText = (type: AccessType) => (
  <>
    <span className={styles.typeLabel}>{`${t(TYPES[type].label)}  `}</span>
    <span className={styles.typeDescription}>{t(TYPES[type].description)}</span>
  </>
);

/** The World's Access: Public, Invitation Only (edited inline) or Password Protected; every change is written at once. */
export function AccessSection({ worldName }: { worldName: string }) {
  const dispatch = useDispatch();
  const permissions = useSelector(state => state.management.worldPermissions);
  const [pending, setPending] = useState<AccessType | null>(null);
  const [password, setPassword] = useState<'create' | 'change' | null>(null);
  const [writing, setWriting] = useState(false);
  const writingRef = useRef(false);

  useEffect(() => {
    void dispatch(fetchWorldPermissions({ worldName }));
  }, [dispatch, worldName]);

  const loaded = permissions.worldName === worldName && permissions.permissions;
  const access = loaded ? permissions.permissions?.access : undefined;
  const actions = useAccessActions(worldName, access);
  if (!access) return null;

  const write = (action: () => Promise<unknown> | undefined): Promise<boolean> => {
    if (writingRef.current) return Promise.resolve(false);
    writingRef.current = true;
    const result = action();
    if (!result) {
      writingRef.current = false;
      return Promise.resolve(true);
    }
    setWriting(true);
    return result
      .then(
        () => true,
        () => false,
      )
      .finally(() => {
        writingRef.current = false;
        setWriting(false);
      });
  };

  const current = access.type as AccessType;
  const owner = permissions.owner.toLowerCase();
  const collaborators = Object.keys(permissions.summary ?? {}).map(address =>
    address.toLowerCase(),
  );
  const invitedWallets =
    access.type === WorldPermissionType.AllowList
      ? access.wallets.filter(
          wallet => wallet.toLowerCase() !== owner && !collaborators.includes(wallet.toLowerCase()),
        )
      : [];
  const invitedCommunities =
    access.type === WorldPermissionType.AllowList ? (access.communities ?? []) : [];
  const erasesSomething =
    (current === WorldPermissionType.AllowList &&
      invitedWallets.length + invitedCommunities.length > 0) ||
    current === WorldPermissionType.SharedSecret;

  const apply = (type: AccessType) => {
    if (type === WorldPermissionType.SharedSecret) {
      setPending(type);
      setPassword('create');
      return;
    }
    setPending(null);
    write(() => actions.changeType(type));
  };

  const pick = (type: AccessType) => {
    if (type === current || writingRef.current) return;
    if (erasesSomething) setPending(type);
    else apply(type);
  };

  const shown = pending && (password || !erasesSomething) ? pending : current;

  return (
    <div className={styles.section}>
      <div className={styles.block}>
        <FieldTitle as="span">{t('world_detail.settings.access.label')}</FieldTitle>
        <FieldSelect<AccessType>
          className={styles.accessSelect}
          value={shown}
          disabled={writing}
          aria-label={t('world_detail.settings.access.label')}
          renderValue={typeText}
          onChange={pick}
        >
          {(Object.keys(TYPES) as AccessType[]).map(type => (
            <SelectOption
              key={type}
              value={type}
              label={t(TYPES[type].label)}
            >
              <OptionCheck checked={type === shown} />
              <span>{typeText(type)}</span>
            </SelectOption>
          ))}
        </FieldSelect>
      </div>
      {current === WorldPermissionType.AllowList && (
        <>
          <InviteForm
            disabled={writing}
            onAddWallet={address => write(() => actions.addWallet(address))}
            onAddCommunity={id => write(() => actions.addCommunity(id))}
            onAddCsv={data => write(() => actions.addCsv(data))}
          />
          <InvitedList
            wallets={invitedWallets}
            communities={invitedCommunities}
            disabled={writing}
            onRemove={removed => write(() => actions.remove(removed))}
          />
        </>
      )}
      {current === WorldPermissionType.SharedSecret && (
        <span className={cx('ChangePassword', styles.changePassword)}>
          <Button
            className="hub-button hub-button--primary"
            onClick={() => setPassword('change')}
          >
            {t('world_detail.settings.password.change_password')}
          </Button>
        </span>
      )}
      <DefaultModal
        open={!!pending && erasesSomething && !password}
        title={t('world_detail.settings.access.change_type_title')}
        body={t('world_detail.settings.access.change_type_warning')}
        onClose={() => setPending(null)}
        secondary={{ label: t('modal.cancel'), onClick: () => setPending(null) }}
        primary={{
          label: t('world_detail.settings.access.continue'),
          state: 'idle',
          onClick: () => pending && apply(pending),
        }}
      />
      {password && (
        <PasswordModal
          open
          mode={password}
          onCancel={() => {
            setPassword(null);
            setPending(null);
          }}
          onConfirm={secret =>
            void write(() => actions.setPassword(secret)).then(done => {
              if (!done) return;
              setPassword(null);
              setPending(null);
            })
          }
        />
      )}
    </div>
  );
}
