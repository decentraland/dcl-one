import { render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { Project } from '/shared/types/projects';
import { flatten } from '/@/modules/store/translation/flatten';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { ProjectStepWrapper } from './component';

vi.mock('/@/hooks/useAuth', () => ({ useAuth: () => ({ wallet: undefined }) }));
vi.mock('#store', () => ({
  useSelector: (select: (state: unknown) => unknown) =>
    select({ ens: { data: {} }, land: { data: [] } }),
}));
vi.mock('/assets/images/scene-thumbnail-fallback.png', () => ({ default: 'fallback.png' }));

const PROJECT = {
  id: 'project-1',
  path: '/scenes/project-1',
  title: 'My Scene',
  thumbnail: '',
  layout: { rows: 1, cols: 1 },
  scene: { base: '0,0', parcels: ['0,0'] },
} as Project;

function renderWrapper(project: Project) {
  render(
    <ProjectStepWrapper
      isWorld={false}
      project={project}
    >
      <p>Step content</p>
    </ProjectStepWrapper>,
  );
  return screen.getByTestId('publish-modal-project-thumbnail');
}

describe('ProjectStepWrapper thumbnail', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  describe('when the scene has a captured thumbnail', () => {
    it('should show the captured picture', () => {
      const thumbnail = renderWrapper({ ...PROJECT, thumbnail: 'c2NlbmU=' });

      expect(thumbnail.style.backgroundImage).toContain('data:image/png;base64,c2NlbmU=');
    });
  });

  describe('when the scene has no thumbnail', () => {
    it('should show the stock picture', () => {
      const thumbnail = renderWrapper(PROJECT);

      expect(thumbnail.style.backgroundImage).toContain('fallback.png');
    });
  });
});
