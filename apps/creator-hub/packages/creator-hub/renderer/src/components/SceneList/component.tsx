import { useCallback } from 'react';
import { useNavigate } from 'react-router';
import { SelectOption } from '@ui/atoms/Select';
import Text from '@ui/atoms/Text';

import { SortBy } from '/shared/types/projects';
import { t } from '/@/modules/store/translation/utils';
import { useWorkspace } from '/@/hooks/useWorkspace';

import Button from '@ui/atoms/Button';
import { Column } from '../Column';
import { FiltersBar } from '../FiltersBar';
import { Row } from '../Row';
import { ToolbarSelect } from '../ToolbarSelect';
import { Projects } from './Projects';
import type { Props } from './types';

import './styles.css';

export function SceneList({ projects, sortBy, onSort }: Props) {
  const { importProject } = useWorkspace();
  const navigate = useNavigate();

  const sort = useCallback(
    (_sortBy: SortBy) => {
      onSort(_sortBy);
    },
    [sortBy, onSort],
  );

  const handleDropdownChange = useCallback(
    (value: SortBy) => {
      sort(value);
    },
    [sort],
  );

  const renderSortDropdown = () => {
    return (
      <ToolbarSelect
        value={sortBy}
        onChange={handleDropdownChange}
      >
        <SelectOption
          className="sort-item"
          value={SortBy.NEWEST}
        >
          {t('scene_list.sort.newest')}
        </SelectOption>
        <SelectOption
          className="sort-item"
          value={SortBy.NAME}
        >
          {t('scene_list.sort.name')}
        </SelectOption>
        <SelectOption
          className="sort-item"
          value={SortBy.SIZE}
        >
          {t('scene_list.sort.size')}
        </SelectOption>
      </ToolbarSelect>
    );
  };

  return (
    <div className="SceneList">
      <Column className="projects-menu">
        <Row>
          <Text variant="h3">{t('scene_list.my_scenes')}</Text>
          <Row className="actions">
            <Button
              variant="text"
              className="hub-button hub-button--secondary action-button import-button"
              startIcon={<i className="icon import-icon" />}
              data-testid="scene-list-import-button"
              onClick={importProject}
            >
              {t('scene_list.import_scene')}
            </Button>
            <Button
              variant="text"
              className="hub-button hub-button--primary action-button templates-button"
              startIcon={<i className="icon template-icon" />}
              data-testid="scene-list-templates-button"
              onClick={() => navigate('/templates')}
            >
              {t('scene_list.templates')}
            </Button>
          </Row>
        </Row>
        {projects.length > 0 ? (
          <FiltersBar>
            <Text variant="h6">{t('scene_list.results', { count: projects.length })}</Text>
            <>
              <p>{t('scene_list.sort_by')}</p>
              {renderSortDropdown()}
            </>
          </FiltersBar>
        ) : null}
      </Column>
      <div className="list">
        <Projects projects={projects} />
      </div>
    </div>
  );
}
