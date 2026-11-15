import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale, t } from '/@/modules/store/translation/utils';

import { expectFocusTrap, pressBackdrop, pressEscape } from '../../../../tests/utils/modalContract';
import { TestProviders } from '../../../../tests/utils/TestProviders';

import { DeleteProject } from './component';
import type { Props } from './types';

const project = { title: 'My Scene' } as Props['project'];

function renderModal() {
  const handlers = { onClose: vi.fn(), onSubmit: vi.fn() };
  render(
    <TestProviders>
      <DeleteProject
        open
        project={project}
        {...handlers}
      />
    </TestProviders>,
  );
  return handlers;
}

describe('DeleteProject', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  afterEach(cleanup);

  it('should be a dialog named by its title', () => {
    renderModal();

    expect(
      screen.getByRole('dialog', { name: t('modal.delete_project.title', { title: 'My Scene' }) }),
    ).toBeTruthy();
  });

  it('should keep focus in the dialog', () => {
    renderModal();

    expectFocusTrap();
  });

  it('should stay open on Escape and on a press on the backdrop', () => {
    const { onClose } = renderModal();

    pressEscape();
    pressBackdrop();

    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('should close from Cancel', () => {
    const { onClose } = renderModal();

    fireEvent.click(screen.getByRole('button', { name: t('modal.cancel') }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('should submit with the files choice and warn when the files go too', () => {
    const { onSubmit } = renderModal();

    fireEvent.click(
      screen.getByRole('checkbox', { name: t('modal.delete_project.files_checkbox') }),
    );
    expect(screen.getByText(t('modal.delete_project.files_warning'))).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: t('modal.confirm') }));

    expect(onSubmit).toHaveBeenCalledWith(project, true);
  });
});
