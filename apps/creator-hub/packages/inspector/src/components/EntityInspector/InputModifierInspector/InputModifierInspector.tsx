import { useCallback } from 'react';
import type { PBInputModifier } from '@dcl/ecs';

import { withSdk } from '../../../hoc/withSdk';
import { useHasComponent } from '../../../hooks/sdk/useHasComponent';
import { useComponentValue } from '../../../hooks/sdk/useComponentValue';
import { analytics, Event } from '../../../lib/logic/analytics';
import { CoreComponents } from '../../../lib/sdk/components';
import { Block } from '../../Block';
import { Container } from '../../Container';
import { CheckboxField, InfoTooltip } from '../../ui';
import type { StandardInput } from './utils';
import { BLOCKABLE, getStandardInput, setStandardInput } from './utils';
import type { Props } from './types';

import styles from './InputModifierInspector.module.css';

export default withSdk<Props>(({ sdk, entity, initialOpen = true }) => {
  const { InputModifier } = sdk.components;
  const hasComponent = useHasComponent(entity, InputModifier);
  const [value, setValue] = useComponentValue<PBInputModifier>(entity, InputModifier);
  const input = getStandardInput(value);

  const handleChange = useCallback(
    (key: keyof StandardInput, checked: boolean) => setValue(setStandardInput(value, key, checked)),
    [value, setValue],
  );

  const handleRemove = useCallback(async () => {
    sdk.operations.removeComponent(entity, InputModifier);
    await sdk.operations.dispatch();
    analytics.track(Event.REMOVE_COMPONENT, { componentName: CoreComponents.INPUT_MODIFIER });
  }, [sdk, entity, InputModifier]);

  if (!hasComponent) return null;

  return (
    <Container
      label="Input Modifier"
      className="InputModifier"
      initialOpen={initialOpen}
      rightContent={
        <InfoTooltip
          text="Freezes the player, or stops them from walking, running, jumping and more, while they are in this scene."
          link="https://docs.decentraland.org/creator/sdk7/interactivity/avatars/locomotion/#restrict-locomotion"
          type="help"
        />
      }
      component={InputModifier}
      entity={entity}
      onRemoveContainer={handleRemove}
    >
      <Block>
        <CheckboxField
          label={
            <>
              Freeze the player{' '}
              <InfoTooltip
                text="No key moves the avatar; the player can still turn the camera. Gravity, moving platforms and the scene's own animations still move them."
                type="help"
              />
            </>
          }
          checked={!!input.disableAll}
          onChange={event => handleChange('disableAll', event.target.checked)}
        />
      </Block>
      <Block label="Block">
        <div className={styles.grid}>
          {BLOCKABLE.map(({ key, label }) => (
            <CheckboxField
              key={key}
              label={label}
              checked={!!input.disableAll || !!input[key]}
              disabled={!!input.disableAll}
              onChange={event => handleChange(key, event.target.checked)}
            />
          ))}
        </div>
      </Block>
      <p className={styles.hint}>Applies only while the player is inside this scene.</p>
    </Container>
  );
});
