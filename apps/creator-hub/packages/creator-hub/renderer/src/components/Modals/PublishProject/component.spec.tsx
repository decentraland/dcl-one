import { useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { flatten } from '/@/modules/store/translation/flatten';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { captureException } from '@sentry/electron/renderer';
import { TestProviders } from '../../../../tests/utils/TestProviders';

import type { Project } from '/shared/types/projects';
import { en } from '/@/modules/store/translation/locales';
import { setCurrentLocale } from '/@/modules/store/translation/utils';

import { tracked } from '../../../../tests/utils/analytics';
import { PublishProject } from './component';
import type { Props } from './types';

const crash = { enabled: true };
let wallet: string | undefined;

vi.mock('@sentry/electron/renderer', () => ({ captureException: vi.fn() }));
vi.mock('#store', () => ({ useDispatch: () => vi.fn() }));
vi.mock('/@/modules/store/ens', () => ({ fetchENSList: vi.fn() }));
// The modal shell is not under test here; the step specs stub it the same way.
vi.mock('./PublishModal', () => ({
  PublishModal: ({ open, children }: { open: boolean; children: React.ReactNode }) =>
    open ? <div>{children}</div> : null,
}));
vi.mock('/@/hooks/useAuth', () => ({ useAuth: () => ({ wallet }) }));
vi.mock('./steps/Initial', () => ({
  Initial: ({ onStep, onClose }: Props) => {
    if (crash.enabled) {
      throw new Error('step failed');
    }
    return (
      <>
        <p>Initial step</p>
        <button onClick={() => onStep('publish-to-world')}>To a World</button>
        <button onClick={onClose}>Close the dialog</button>
      </>
    );
  },
}));

vi.mock('./steps/AlternativeServers', () => ({ AlternativeServers: () => null }));
vi.mock('./steps/PublishToWorld', () => ({
  PublishToWorld: function WorldStep() {
    const [selected, setSelected] = useState(false);
    return (
      <button onClick={() => setSelected(true)}>
        {selected ? 'Old World selected' : 'Select a World'}
      </button>
    );
  },
}));
vi.mock('./steps/PublishToLand', () => ({ PublishToLand: () => null }));
vi.mock('./steps/Deploy', () => ({ Deploy: () => null }));

const project = { id: 'project-1', title: 'My Scene' } as Project;

function renderModal(onClose = vi.fn(), initialStep?: Props['initialStep']) {
  const result = render(
    <TestProviders>
      <p>Editor behind the modal</p>
      <PublishProject
        open
        project={project}
        onClose={onClose}
        initialStep={initialStep}
      />
    </TestProviders>,
  );
  return { onClose, ...result };
}

describe('PublishProject error boundary', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    crash.enabled = true;
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('when a step throws during render', () => {
    let onClose: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      ({ onClose } = renderModal());
    });

    it('should show the fallback inside the modal', () => {
      expect(screen.getByTestId('publish-error-fallback')).toBeTruthy();
    });

    it('should keep the page behind the modal mounted', () => {
      expect(screen.getByText('Editor behind the modal')).toBeTruthy();
    });

    it('should report the error to Sentry', () => {
      expect(captureException).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'step failed' }),
        expect.objectContaining({
          tags: { source: 'error-boundary', boundary: 'publish-modal' },
        }),
      );
    });

    describe('and the user clicks Close', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Close' }));
      });

      it('should close the modal', () => {
        expect(onClose).toHaveBeenCalled();
      });
    });

    describe('and the user clicks Try again once the cause is gone', () => {
      beforeEach(() => {
        crash.enabled = false;
        fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
      });

      it('should render the step again', () => {
        expect(screen.getByText('Initial step')).toBeTruthy();
        expect(screen.queryByTestId('publish-error-fallback')).toBeNull();
      });
    });
  });

  describe('when the modal is reopened after an error', () => {
    it('should start from the step instead of the fallback', () => {
      const onClose = vi.fn();
      const { rerender } = renderModal(onClose);
      expect(screen.getByTestId('publish-error-fallback')).toBeTruthy();

      crash.enabled = false;
      const modal = (open: boolean) => (
        <TestProviders>
          <p>Editor behind the modal</p>
          <PublishProject
            open={open}
            project={project}
            onClose={onClose}
          />
        </TestProviders>
      );
      rerender(modal(false));
      rerender(modal(true));

      expect(screen.getByText('Initial step')).toBeTruthy();
    });
  });
});

it('starts a fresh World picker when the wallet changes while publishing', () => {
  const onClose = vi.fn();
  const modal = () => (
    <TestProviders>
      <PublishProject
        open
        project={project}
        onClose={onClose}
        initialStep="publish-to-world"
      />
    </TestProviders>
  );
  wallet = '0xa';
  const { rerender } = render(modal());
  try {
    fireEvent.click(screen.getByRole('button', { name: 'Select a World' }));
    expect(screen.getByRole('button', { name: 'Old World selected' })).toBeTruthy();

    wallet = '0xb';
    rerender(modal());

    expect(screen.getByRole('button', { name: 'Select a World' })).toBeTruthy();
  } finally {
    wallet = undefined;
    cleanup();
  }
});

describe('PublishProject analytics', () => {
  beforeAll(() => {
    setCurrentLocale('en', flatten(en));
  });

  beforeEach(() => {
    crash.enabled = false;
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  describe('when the dialog opens', () => {
    beforeEach(() => {
      renderModal();
    });

    it('should track the opening with its first step and the sign-in state', () => {
      expect(tracked('Publish Modal Opened')).toEqual([
        { project_id: 'project-1', initial_step: 'initial', signed_in: false },
      ]);
    });

    it('should track the first step as viewed', () => {
      expect(tracked('Publish Step Viewed')).toEqual([
        { project_id: 'project-1', step: 'initial' },
      ]);
    });

    describe('and the creator picks a destination', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'To a World' }));
      });

      it('should track the step it leads to', () => {
        expect(tracked('Publish Step Viewed')).toContainEqual({
          project_id: 'project-1',
          step: 'publish-to-world',
        });
      });
    });

    describe('and the creator closes it', () => {
      beforeEach(() => {
        fireEvent.click(screen.getByRole('button', { name: 'Close the dialog' }));
      });

      it('should track the step it was closed on', () => {
        expect(tracked('Publish Modal Closed')).toEqual([
          { project_id: 'project-1', step: 'initial' },
        ]);
      });
    });
  });

  describe('when the dialog reopens on the deploy step', () => {
    beforeEach(() => {
      renderModal(vi.fn(), 'deploy');
    });

    it('should track the opening with the step it opened on', () => {
      expect(tracked('Publish Modal Opened')).toEqual([
        { project_id: 'project-1', initial_step: 'deploy', signed_in: false },
      ]);
    });

    it('should not count the deploy step as viewed again', () => {
      expect(tracked('Publish Step Viewed')).toEqual([]);
    });
  });
});
