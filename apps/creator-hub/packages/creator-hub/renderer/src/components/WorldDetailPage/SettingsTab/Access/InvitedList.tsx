import { useState } from 'react';
import { cx } from '/@/lib/cx';
import AvatarFace, { type AvatarFaceProps } from '@ui/atoms/AvatarFace';
import IconButton from '@ui/atoms/IconButton';

import { useCommunities } from '/@/hooks/useCommunities';
import { useProfile } from '/@/hooks/useProfile';
import { getCommunityThumbnailUrl } from '/@/lib/communities';
import { t } from '/@/modules/store/translation/utils';

import { DeleteIcon } from '../../../Icons';
import { ListHeader, Row, Rows } from '../../../LandDetailPage/access/access.styled';
import { BulkTrash } from '../../../LandDetailPage/access/BulkTrash';
import { useDisplayName } from '../../../LandDetailPage/access/useDisplayName';
import { SelectCheckbox } from '../../../LandDetailPage/SelectCheckbox';
import { DefaultModal } from '../../../Modals/DefaultModal';

import styles from './Access.module.css';

type Invite = { kind: 'wallet' | 'community'; id: string; name: string };
type Removal = { wallets: string[]; communities: string[] };

type Props = {
  wallets: string[];
  communities: string[];
  disabled: boolean;
  onRemove: (removal: Removal) => Promise<boolean>;
};

const keyOf = (invite: Invite) => `${invite.kind}:${invite.id}`;

function WalletFace({ address }: { address: string }) {
  const { avatar } = useProfile(address);
  return (
    <AvatarFace
      avatar={avatar as AvatarFaceProps['avatar']}
      className={styles.avatar}
      size="small"
      inline
    />
  );
}

const toRemoval = (invites: Invite[]): Removal => ({
  wallets: invites.filter(invite => invite.kind === 'wallet').map(invite => invite.id),
  communities: invites.filter(invite => invite.kind === 'community').map(invite => invite.id),
});

/** The World's invites, wallets first then communities, removable one by one or in bulk after a confirm. */
export function InvitedList({ wallets, communities, disabled, onRemove }: Props) {
  const displayName = useDisplayName();
  const { communities: resolved } = useCommunities(communities);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState<Invite[] | null>(null);
  const [busy, setBusy] = useState(false);

  const invites: Invite[] = [
    ...wallets.map(id => ({ kind: 'wallet' as const, id, name: displayName(id) })),
    ...communities.map(id => ({
      kind: 'community' as const,
      id,
      name: resolved.get(id)?.name ?? id,
    })),
  ];
  const chosen = invites.filter(invite => selected.has(keyOf(invite)));
  const all = invites.length > 0 && chosen.length === invites.length;

  const toggle = (key: string) =>
    setSelected(current => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  const confirm = async () => {
    if (!removing) return;
    setBusy(true);
    try {
      if (!(await onRemove(toRemoval(removing)))) return;
      setSelected(current => {
        const next = new Set(current);
        removing.forEach(invite => next.delete(keyOf(invite)));
        return next;
      });
      setRemoving(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.list}>
      <ListHeader>
        <span className="Lead">
          {invites.length > 0 && (
            <SelectCheckbox
              variant="secondary"
              checked={all}
              indeterminate={chosen.length > 0 && !all}
              onChange={() => setSelected(all ? new Set() : new Set(invites.map(keyOf)))}
              inputProps={{ 'aria-label': t('world_detail.settings.access.select_all') }}
            />
          )}
          {t('world_detail.settings.access.invited')}
          {chosen.length > 0 && <BulkTrash onClick={() => setRemoving(chosen)} />}
        </span>
      </ListHeader>
      {invites.length ? (
        <Rows>
          {invites.map(invite => (
            <Row key={keyOf(invite)}>
              <span className="Lead">
                <SelectCheckbox
                  variant="secondary"
                  checked={selected.has(keyOf(invite))}
                  onChange={() => toggle(keyOf(invite))}
                  inputProps={{
                    'aria-label': t('world_detail.settings.access.select', { name: invite.name }),
                  }}
                />
                <span className={styles.who}>
                  {invite.kind === 'wallet' ? (
                    <WalletFace address={invite.id} />
                  ) : (
                    <img
                      src={getCommunityThumbnailUrl(invite.id)}
                      alt=""
                    />
                  )}
                  <span className={cx('Name', styles.name)}>{invite.name}</span>
                </span>
              </span>
              <IconButton
                className={cx('Trash', styles.trash)}
                aria-label={t('world_detail.settings.access.remove', { name: invite.name })}
                disabled={disabled}
                onClick={() => setRemoving([invite])}
              >
                <DeleteIcon />
              </IconButton>
            </Row>
          ))}
        </Rows>
      ) : (
        <p className={styles.empty}>{t('world_detail.settings.access.invited_empty')}</p>
      )}
      <DefaultModal
        open={!!removing}
        title={
          removing && removing.length > 1
            ? t('world_detail.settings.access.remove_many_title', { count: removing.length })
            : t('land_detail.collaborators.remove_title', { name: removing?.[0]?.name ?? '' })
        }
        body={t('world_detail.settings.access.remove_body')}
        onClose={() => setRemoving(null)}
        secondary={{ label: t('modal.cancel'), onClick: () => setRemoving(null) }}
        primary={{
          label: t('land_detail.collaborators.remove_confirm'),
          state: busy || disabled ? 'busy' : 'idle',
          onClick: () => void confirm(),
        }}
      />
    </div>
  );
}
