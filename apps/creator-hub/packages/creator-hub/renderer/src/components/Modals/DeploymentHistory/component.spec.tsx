import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../../tests/utils/modalContract';
import { TestProviders } from '../../../../tests/utils/TestProviders';

import { DeploymentHistory } from './component';

vi.mock('/@/hooks/useDeploy', () => ({
  useDeploy: () => ({
    getDeployment: () => undefined,
    getDeploymentHistory: () => [],
    deriveOverallStatus: () => 'idle',
  }),
}));

function renderModal() {
  const onClose = vi.fn();
  render(
    <TestProviders>
      <DeploymentHistory
        open
        projectPath="/scenes/my-scene"
        onClose={onClose}
      />
    </TestProviders>,
  );
  return { onClose };
}

describe('DeploymentHistory', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  it('should be a dialog named by its title that tells when there is no deployment', () => {
    renderModal();

    expect(screen.getByRole('dialog', { name: t('modal.deployment_history.title') })).toBeTruthy();
    expect(screen.getByText(t('modal.deployment_history.no_deployments'))).toBeTruthy();
  });

  it('should keep focus in the dialog', () => {
    renderModal();

    expectFocusTrap();
  });

  it('should close from the close button, from Escape and from a press on the backdrop', () => {
    const { onClose } = renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    pressEscape();
    expect(onClose).toHaveBeenCalledTimes(2);
    pressBackdrop();
    expect(onClose).toHaveBeenCalledTimes(3);
  });
});
