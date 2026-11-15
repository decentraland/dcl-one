import { cx } from '/@/lib/cx';

import type { WorldSettings } from '/@/lib/worlds';
import { t } from '/@/modules/store/translation/utils';

import { FieldTitle } from '../../LandDetailPage/DetailsTab/DetailsTab.styled';
import { SelectCheckbox } from '../../LandDetailPage/SelectCheckbox';
import { Loader } from '../../Loader';
import { TextInput } from '../../TextInput';
import { useWorldSettings } from '../useWorldSettings';
import { useWorldSettingsDraft } from '../useWorldSettingsDraft';
import type { WorldPermissions } from '../worldPermissions';
import { AccessSection } from './Access';
import { CheckRow } from './CheckRow';
import { settingsChanges, settingsValid, toSettingsDraft } from './settings';
import { SkyboxField } from './SkyboxField';

import styles from './SettingsTab.module.css';

type Props = { worldName: string; access: WorldPermissions['access'] };

function SettingsForm({ worldName, access, saved }: Props & { saved: WorldSettings }) {
  const [draft, setDraft] = useWorldSettingsDraft({
    worldName,
    saved,
    toDraft: toSettingsDraft,
    changesOf: settingsChanges,
    valid: current => settingsValid(current) && (!!current.x || !saved.spawnCoordinates),
  });
  const edit = (patch: Partial<typeof draft>) => setDraft(current => ({ ...current, ...patch }));

  const coordinate = (axis: 'x' | 'y') => (
    <TextInput
      className={styles.smallInput}
      value={draft[axis]}
      startAdornment={<span className={styles.prefix}>{axis.toUpperCase()}</span>}
      inputProps={{ 'aria-label': axis.toUpperCase(), inputMode: 'numeric' }}
      onChange={event => edit({ [axis]: event.target.value.trim() })}
    />
  );

  return (
    <div className={styles.root}>
      <div className={styles.section}>
        <FieldTitle as="span">{t('world_detail.settings.spawn')}</FieldTitle>
        <div className={cx(styles.row, styles.tight)}>
          {coordinate('x')}
          {coordinate('y')}
        </div>
      </div>
      <SkyboxField
        value={draft.skyboxTime}
        onChange={skyboxTime => edit({ skyboxTime })}
      />
      <div className={styles.section}>
        <FieldTitle as="span">{t('world_detail.settings.general')}</FieldTitle>
        <CheckRow>
          <SelectCheckbox
            checked={draft.singlePlayer}
            onChange={event => edit({ singlePlayer: event.target.checked })}
          />
          {t('world_detail.settings.single_player')}
        </CheckRow>
        <p className={styles.note}>{t('world_detail.settings.single_player_streaming_note')}</p>
        <CheckRow>
          <SelectCheckbox
            checked={draft.showInPlaces}
            onChange={event => edit({ showInPlaces: event.target.checked })}
          />
          {t('world_detail.settings.discoverable')}
        </CheckRow>
      </div>
      {access === 'manage' && <AccessSection worldName={worldName} />}
    </div>
  );
}

/** The World's Settings: spawn, skybox and General saved from the shell's SAVE; Access, for the owner, saved as it changes. */
export function SettingsTab({ worldName, access }: Props) {
  const settings = useWorldSettings(worldName);
  if (settings === 'failed') return null;
  if (settings === 'loading') return <Loader size={70} />;
  return (
    <SettingsForm
      worldName={worldName}
      access={access}
      saved={settings}
    />
  );
}
