import { useCallback, useRef, useState } from 'react';
import Button from '@ui/atoms/Button';
import { Menu, MenuItem } from '@ui/atoms/Menu';
import Tooltip from '@ui/atoms/Tooltip';
import { AddIcon, FileDownloadOutlinedIcon } from '@ui/atoms/icons-material';

import arrowDown from '/assets/images/arrow-down.svg';
import { useMatchMedia } from '/@/hooks/useMatchMedia';
import { useWorkspace } from '/@/hooks/useWorkspace';
import { t } from '/@/modules/store/translation/utils';
import { Plus } from '../Icons/figma';
import { CreateProject } from '../Modals/CreateProject';

import { ICON_ONLY_QUERY } from './breakpoints';
import type { CreateProjectValue, NewProjectPayload } from './types';
import styles from './component.module.css';

export function CreateButton() {
  const { createProject, getAvailableProject, importProject } = useWorkspace();
  const anchorRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [newProject, setNewProject] = useState<NewProjectPayload | undefined>();
  const iconOnly = useMatchMedia(ICON_ONLY_QUERY);

  const handleToggleMenu = useCallback(() => setOpen(current => !current), []);
  const handleCloseMenu = useCallback(() => setOpen(false), []);

  const handleClickNewScene = useCallback(async () => {
    setOpen(false);
    const [error, data] = await getAvailableProject();
    if (error) return;
    const { name, path } = data;
    setNewProject({
      name,
      path: path.endsWith(name) ? path.slice(0, -name.length) : path,
    });
  }, [getAvailableProject]);

  const handleClickImportScene = useCallback(() => {
    setOpen(false);
    importProject();
  }, [importProject]);

  const handleCreateProject = useCallback(
    (value: CreateProjectValue) => {
      if (!newProject) return;
      createProject({ ...newProject, ...value });
      setNewProject(undefined);
    },
    [createProject, newProject],
  );

  return (
    <>
      <div
        className={styles.row}
        ref={anchorRef}
      >
        <Tooltip
          label={t('navbar.create.label')}
          side="bottom"
          tipClassName="AccountTooltip"
          portal
          disabled={!iconOnly}
        >
          <Button
            className={styles.label}
            data-testid="create-button"
            startIcon={<Plus fontSize="inherit" />}
            onClick={handleClickNewScene}
          >
            <span className={styles.labelText}>{t('navbar.create.label')}</span>
          </Button>
        </Tooltip>
        <Button
          className={styles.toggle}
          data-testid="create-button-toggle"
          aria-label={t('navbar.create.options')}
          aria-haspopup="menu"
          aria-expanded={open ? 'true' : undefined}
          onClick={handleToggleMenu}
        >
          <img
            src={arrowDown}
            alt=""
            width={11}
            height={8}
          />
        </Button>
      </div>
      <Menu
        className={styles.menu}
        anchor={anchorRef.current}
        open={open}
        onClose={handleCloseMenu}
        placement="bottom-end"
        offset={4}
      >
        <MenuItem
          className={styles.item}
          data-testid="create-button-new-scene"
          icon={<AddIcon fontSize="small" />}
          onClick={handleClickNewScene}
        >
          {t('navbar.create.new_scene')}
        </MenuItem>
        <MenuItem
          className={styles.item}
          data-testid="create-button-import-scene"
          icon={<FileDownloadOutlinedIcon fontSize="small" />}
          onClick={handleClickImportScene}
        >
          {t('navbar.create.import_scene')}
        </MenuItem>
      </Menu>
      {newProject && (
        <CreateProject
          open
          initialValue={newProject}
          onClose={() => setNewProject(undefined)}
          onSubmit={handleCreateProject}
        />
      )}
    </>
  );
}
