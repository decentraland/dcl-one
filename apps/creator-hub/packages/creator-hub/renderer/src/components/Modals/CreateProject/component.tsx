import { useCallback, useState } from 'react';
import IconButton from '@ui/atoms/IconButton';
import { FolderIcon } from '@ui/atoms/icons-material';
import Spinner from '@ui/atoms/HubSpinner';
import Text from '@ui/atoms/Text';
import TextField from '@ui/atoms/TextField';

import { t } from '/@/modules/store/translation/utils';

import { useWorkspace } from '/@/hooks/useWorkspace';

import { PublishModal as Modal } from '../PublishProject/PublishModal';
import { Button } from '../../Button';

import type { Props, Value } from './types';

import './styles.css';

export function CreateProject({ open, initialValue, onClose, onSubmit }: Props) {
  const { validateProjectPath, selectNewProjectPath } = useWorkspace();
  const [value, setValue] = useState<Value>(initialValue);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Runs on blur too, so it must never disable the Create button: pressing the
  // button blurs the input first (mousedown → blur → click), and a button disabled
  // by that blur swallows the click, forcing a second one (#832).
  const validate = useCallback(async () => {
    const result = await validateProjectPath(value.path, value.name);
    if (result === 'invalid-dir') {
      // The Path field expects an existing folder (via the picker) — a bare/typed
      // name isn't a valid directory. Say so, instead of the misleading "exists".
      setError(t('modal.create_project.errors.invalid_directory'));
    } else if (result === 'path-taken') {
      setError(t('modal.create_project.errors.path_exists_or_not_writable'));
    }
    return result === true;
  }, [value, validateProjectPath]);

  const handleChange = useCallback(
    (key: keyof Value) => (event: React.ChangeEvent<HTMLInputElement>) => {
      setError(null);
      setValue(prev => ({ ...prev, [key]: event.target.value }));
    },
    [],
  );

  const handleOpenFolder = useCallback(async () => {
    setError(null);
    setLoading(true);
    const folder = await selectNewProjectPath();
    setLoading(false);
    if (folder) setValue({ ...value, path: folder });
  }, [value]);

  const handleSubmit = useCallback(async () => {
    setLoading(true);
    const valid = await validate();
    setLoading(false);
    if (valid) onSubmit(value);
  }, [onSubmit, value, validate]);

  return (
    <Modal
      open={open}
      title={t('modal.create_project.title')}
      onClose={onClose}
      size="medium"
      actions={
        <>
          <Button
            color="secondary"
            data-testid="create-project-modal-cancel-button"
            onClick={onClose}
          >
            {t('modal.cancel')}
          </Button>
          <Button
            data-testid="create-project-modal-create-button"
            onClick={handleSubmit}
            disabled={loading || !!error}
          >
            {loading ? (
              <Spinner
                size={20}
                color="primary"
                role="progressbar"
              />
            ) : (
              t('modal.create_project.actions.create')
            )}
          </Button>
        </>
      }
    >
      <div
        className="CreateProjectModal"
        data-testid="create-project-modal"
      >
        <div className="CreateProjectFormControl">
          <Text variant="body1">{t('modal.create_project.fields.name')}</Text>
          <TextField
            className="CreateProjectField"
            tone="neutral"
            fullWidth
            data-testid="create-project-modal-name-input"
            value={value.name}
            onChange={handleChange('name')}
            onBlur={validate}
          />
          <Text variant="body1">{t('modal.create_project.fields.path')}</Text>
          <TextField
            className="CreateProjectField"
            tone="neutral"
            fullWidth
            data-testid="create-project-modal-path-input"
            value={value.path}
            onChange={handleChange('path')}
            onBlur={validate}
            endAdornment={
              <IconButton
                aria-label={t('modal.app_settings.fields.scenes_folder.change_button')}
                onClick={handleOpenFolder}
                edge="end"
              >
                <FolderIcon />
              </IconButton>
            }
          />
          {error && (
            <Text
              variant="body1"
              className="error"
              data-testid="create-project-modal-error"
            >
              {error}
            </Text>
          )}
        </div>
      </div>
    </Modal>
  );
}
