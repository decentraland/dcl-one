import { useCallback } from 'react';
import Text from '@ui/atoms/Text';

import { RENDERER } from '/shared/types/settings';
import { t } from '/@/modules/store/translation/utils';
import { IS_NON_PRODUCTION_BUILD } from '/@/modules/buildMode';
import { ToggleRow } from '../../ToggleRow';
import type { BaseTabProps } from '../../types';

import './styles.css';

const ExperimentalTab = ({ settings, updateSettings }: BaseTabProps) => {
  const handleRendererChange = useCallback(
    (bevy: boolean) => {
      updateSettings({ ...settings, renderer: bevy ? RENDERER.BEVY : RENDERER.BABYLON });
    },
    [settings, updateSettings],
  );

  const handleDevAssetCatalogChange = useCallback(
    (checked: boolean) => {
      updateSettings({ ...settings, useDevAssetCatalog: checked });
    },
    [settings, updateSettings],
  );

  return (
    <div className="FormContainer ExperimentalTab">
      <div className="ExperimentalField">
        <ToggleRow
          checked={settings.renderer === RENDERER.BEVY}
          label={t('modal.app_settings.fields.renderer.toggle')}
          onChange={handleRendererChange}
        />
      </div>
      {/* Dev/QA only: shown solely in non-production builds so unreleased smart items can be tested
          from the dev CDN. A production release never renders this. */}
      {IS_NON_PRODUCTION_BUILD && (
        <div className="ExperimentalField">
          <ToggleRow
            checked={!!settings.useDevAssetCatalog}
            label={t('modal.app_settings.fields.dev_asset_catalog.toggle')}
            onChange={handleDevAssetCatalogChange}
          />
          <Text
            variant="body2"
            tone="secondary"
            className="ExperimentalHint"
          >
            {t('modal.app_settings.fields.dev_asset_catalog.help')}
          </Text>
        </div>
      )}
    </div>
  );
};

export default ExperimentalTab;
