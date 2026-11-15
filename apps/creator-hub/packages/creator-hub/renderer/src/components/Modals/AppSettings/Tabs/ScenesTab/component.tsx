import { useCallback } from 'react';
import { FolderIcon } from '/@/components/Icons/material';
import Button from '@ui/atoms/HubButton';
import IconButton from '@ui/atoms/IconButton';
import Radio, { RadioGroup } from '@ui/atoms/Radio';
import Text from '@ui/atoms/Text';
import TextField from '@ui/atoms/TextField';

import { DEPENDENCY_UPDATE_STRATEGY } from '/shared/types/settings';
import { t } from '/@/modules/store/translation/utils';
import type { ScenesTabProps } from '../../types';

import './styles.css';

const ScenesTab: React.FC<ScenesTabProps> = ({
  settings,
  updateSettings,
  error,
  isCustomScenesPath,
  onOpenFolder,
  onResetScenesFolder,
  onValidateScenesPath,
}) => {
  const handleChangeSceneFolder = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const newSettings = { ...settings, scenesPath: event.target.value };
      updateSettings(newSettings);
      onValidateScenesPath(newSettings.scenesPath);
    },
    [settings, updateSettings, onValidateScenesPath],
  );

  const handleChangeUpdateDependenciesStrategy = useCallback(
    (value: string) => {
      const newSettings = {
        ...settings,
        dependencyUpdateStrategy: value as DEPENDENCY_UPDATE_STRATEGY,
      };
      updateSettings(newSettings);
    },
    [settings, updateSettings],
  );

  return (
    <div className="FormContainer">
      <div className="FormGroup ScenesFolderFormControl">
        <Text
          variant="body1"
          id="scenes-folder-label"
        >
          {t('modal.app_settings.fields.scenes_folder.label')}
        </Text>
        <div className="ScenesFolderInputContainer">
          <TextField
            tone="neutral"
            aria-labelledby="scenes-folder-label"
            value={settings.scenesPath}
            onChange={handleChangeSceneFolder}
            error={!!error}
            fullWidth
            endAdornment={
              <span className="ScenesFolderInputAdornment">
                {isCustomScenesPath && (
                  <Button
                    variant="text"
                    tone="neutral"
                    onClick={onResetScenesFolder}
                    className="ResetScenesFolderButton"
                  >
                    {t('modal.app_settings.fields.scenes_folder.reset_button')}
                  </Button>
                )}
                <IconButton
                  onClick={onOpenFolder}
                  edge="end"
                  aria-label={t('modal.app_settings.fields.scenes_folder.change_button')}
                >
                  <FolderIcon />
                </IconButton>
              </span>
            }
          />
        </div>
        {error && (
          <Text
            variant="body1"
            className="error"
          >
            {error}
          </Text>
        )}
      </div>
      <div className="FormGroup DependencyUpdateFormGroup">
        <Text
          variant="body1"
          id="scene-dependencies-label"
        >
          {t('modal.app_settings.fields.scene_editor_dependencies.label')}
        </Text>
        <RadioGroup
          className="SettingsRadioGroup"
          aria-labelledby="scene-dependencies-label"
          value={settings.dependencyUpdateStrategy}
          onChange={handleChangeUpdateDependenciesStrategy}
        >
          <Radio
            className="SettingsRadio"
            value={DEPENDENCY_UPDATE_STRATEGY.AUTO_UPDATE}
          >
            {t('modal.app_settings.fields.scene_editor_dependencies.options.auto_update')}
          </Radio>
          <Radio
            className="SettingsRadio"
            value={DEPENDENCY_UPDATE_STRATEGY.NOTIFY}
          >
            {t('modal.app_settings.fields.scene_editor_dependencies.options.notify')}
          </Radio>
          <Radio
            className="SettingsRadio"
            value={DEPENDENCY_UPDATE_STRATEGY.DO_NOTHING}
          >
            {t('modal.app_settings.fields.scene_editor_dependencies.options.do_nothing')}
          </Radio>
        </RadioGroup>
      </div>
    </div>
  );
};

export default ScenesTab;
