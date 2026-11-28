import { useCallback } from 'react';
import type { PBAvatarLocomotionSettings } from '@dcl/ecs';

import { withSdk } from '../../../hoc/withSdk';
import { useHasComponent } from '../../../hooks/sdk/useHasComponent';
import { useComponentValue } from '../../../hooks/sdk/useComponentValue';
import { analytics, Event } from '../../../lib/logic/analytics';
import { CoreComponents } from '../../../lib/sdk/components';
import { Block } from '../../Block';
import { Container } from '../../Container';
import { InfoTooltip, TextField } from '../../ui';
import type { LocomotionKey } from './utils';
import { LOCOMOTION_GROUPS, setLocomotionSetting } from './utils';
import type { Props } from './types';

import styles from './AvatarLocomotionSettingsInspector.module.css';

export default withSdk<Props>(({ sdk, entity, initialOpen = true }) => {
  const { AvatarLocomotionSettings } = sdk.components;
  const hasComponent = useHasComponent(entity, AvatarLocomotionSettings);
  const [value, setValue] = useComponentValue<PBAvatarLocomotionSettings>(
    entity,
    AvatarLocomotionSettings,
  );

  const handleChange = useCallback(
    (key: LocomotionKey, text: string) => {
      const next = setLocomotionSetting(value ?? {}, key, text);
      if (next) setValue(next);
    },
    [value, setValue],
  );

  const handleRemove = useCallback(async () => {
    sdk.operations.removeComponent(entity, AvatarLocomotionSettings);
    await sdk.operations.dispatch();
    analytics.track(Event.REMOVE_COMPONENT, {
      componentName: CoreComponents.AVATAR_LOCOMOTION_SETTINGS,
    });
  }, [sdk, entity, AvatarLocomotionSettings]);

  if (!hasComponent) return null;

  return (
    <Container
      label="Locomotion Settings"
      className="AvatarLocomotionSettings"
      initialOpen={initialOpen}
      rightContent={
        <InfoTooltip
          text="Changes how fast the player moves and how high they jump while they are in this scene."
          link="https://docs.decentraland.org/creator/sdk7/interactivity/avatars/locomotion/"
          type="help"
        />
      }
      component={AvatarLocomotionSettings}
      entity={entity}
      onRemoveContainer={handleRemove}
    >
      {LOCOMOTION_GROUPS.map(group => (
        <Block
          key={group.label}
          label={`${group.label} (${group.unit})`}
        >
          {group.fields.map(field => (
            <TextField
              key={field.key}
              type="number"
              min={0}
              step="any"
              leftLabel={field.label}
              placeholder={String(field.fallback)}
              value={value?.[field.key] ?? ''}
              onChange={event => handleChange(field.key, event.target.value)}
              autoSelect
            />
          ))}
        </Block>
      ))}
      <p className={styles.hint}>
        Empty fields keep the default, shown in grey. Values apply only while the player is inside
        this scene.
      </p>
    </Container>
  );
});
