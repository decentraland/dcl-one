import { useCallback, useId, useState } from 'react';
import Checkbox from '@ui/atoms/Checkbox';
import IconButton from '@ui/atoms/IconButton';
import Select, { SelectOption } from '@ui/atoms/Select';
import Spinner from '@ui/atoms/HubSpinner';
import Text from '@ui/atoms/Text';

import { CheckIcon, DeleteIcon } from '/@/components/Icons/material';

import type { EditorConfig } from '/shared/types/config';
import { t } from '/@/modules/store/translation/utils';
import type { EditorTabProps } from '../../types';

import './styles.css';

const EditorTab: React.FC<EditorTabProps> = ({
  settings,
  updateSettings,
  editors,
  loading,
  onSetDefaultEditor,
  onAddEditor,
  onRemoveEditor,
  onSelectEditorPath,
}) => {
  const editorLabelId = useId();
  const [selectOpen, setSelectOpen] = useState(false);

  const handleEditorChange = useCallback(
    (selectedPath: string) => {
      if (selectedPath) {
        onSetDefaultEditor(selectedPath);
      }
    },
    [onSetDefaultEditor],
  );

  const handleAddCustomEditor = useCallback(async () => {
    const editorPath = await onSelectEditorPath();
    if (editorPath) {
      onAddEditor(editorPath);
    }
  }, [onSelectEditorPath, onAddEditor]);

  const handlePreviewOptionChange = useCallback(
    (option: keyof typeof settings.previewOptions, checked: boolean) => {
      const newSettings = {
        ...settings,
        previewOptions: {
          ...settings.previewOptions,
          [option]: checked,
        },
      };
      updateSettings(newSettings);
    },
    [settings, updateSettings],
  );

  return (
    <div className="FormContainer">
      <div className="FormGroup CodeEditorFormGroup">
        <Text
          variant="body1"
          id={editorLabelId}
        >
          {t('modal.app_settings.fields.code_editor.label')}
        </Text>
        {loading ? (
          <Spinner
            ring="wide"
            size={24}
            color="primary"
            role="progressbar"
          />
        ) : (
          <Select
            fullWidth
            className="CodeEditorSelect"
            menuClassName="CodeEditorMenu"
            aria-labelledby={editorLabelId}
            open={selectOpen}
            onOpenChange={setSelectOpen}
            placeholder="Add or select a default editor"
            value={(editors && editors.find(e => e.isDefault)?.path) || ''}
            renderValue={value =>
              (editors && editors.find(e => e.path === value)?.name) ||
              'Add or select a default editor'
            }
            onChange={handleEditorChange}
          >
            {(editors || []).map((editor: EditorConfig) => (
              <SelectOption
                key={editor.path}
                value={editor.path}
                label={editor.name}
                className="editor-select"
              >
                <span>{editor.name}</span>
                <div className="EditorActionsBox">
                  {editor.isDefault && <CheckIcon className="default-icon menu-only" />}
                  <IconButton
                    size="small"
                    aria-label={t('modal.app_settings.fields.code_editor.remove', {
                      name: editor.name,
                    })}
                    onClick={e => {
                      e.stopPropagation();
                      onRemoveEditor(editor.path);
                    }}
                  >
                    <DeleteIcon fontSize="small" />
                  </IconButton>
                </div>
              </SelectOption>
            ))}
            <div
              role="option"
              aria-selected={false}
              className="custom-editor"
              onMouseDown={async (e: React.MouseEvent) => {
                e.preventDefault();
                await handleAddCustomEditor();
              }}
              onClick={() => setSelectOpen(false)}
            >
              {t('modal.app_settings.fields.code_editor.choose_device')}
            </div>
          </Select>
        )}
      </div>
      <div className="FormGroup PreviewOptionsFormGroup">
        <Text variant="body1">{t('editor.header.actions.preview_options.title')}</Text>
        <Checkbox
          className="SettingsCheckbox"
          checked={!!settings.previewOptions.debugger}
          onChange={checked => handlePreviewOptionChange('debugger', checked)}
        >
          {t('editor.header.actions.preview_options.debugger')}
        </Checkbox>
        <Checkbox
          className="SettingsCheckbox"
          checked={!!settings.previewOptions.enableLandscapeTerrains}
          onChange={checked => handlePreviewOptionChange('enableLandscapeTerrains', checked)}
        >
          {t('editor.header.actions.preview_options.landscape_terrain_enabled')}
        </Checkbox>
        <Checkbox
          className="SettingsCheckbox"
          checked={!!settings.previewOptions.loadRawGltfs}
          onChange={checked => handlePreviewOptionChange('loadRawGltfs', checked)}
        >
          {t('editor.header.actions.preview_options.load_raw_gltfs')}
        </Checkbox>
      </div>
      <div className="FormGroup AppWarningsFormGroup">
        <Text variant="body1">{t('modal.app_settings.fields.app_warnings.label')}</Text>
        <Checkbox
          className="SettingsCheckbox"
          checked={!!settings.previewOptions.showWarnings}
          onChange={checked => handlePreviewOptionChange('showWarnings', checked)}
        >
          {t('modal.app_settings.fields.app_warnings.show_warnings')}
        </Checkbox>
      </div>
    </div>
  );
};

export default EditorTab;
