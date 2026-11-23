import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { PublishModal } from '.';

describe('PublishModal', () => {
  afterEach(cleanup);

  describe('when the step has no previous step', () => {
    it('should show no back button, so the title starts at the edge of the fields', () => {
      render(
        <PublishModal
          open
          title="Create project"
          onClose={vi.fn()}
        >
          <p>Fields</p>
        </PublishModal>,
      );
      expect(screen.queryByRole('button', { name: 'back' })).toBeNull();
    });
  });

  describe('when the step has a previous step', () => {
    it('should call onBack from the back button', () => {
      const onBack = vi.fn();
      render(
        <PublishModal
          open
          title="Publish"
          onBack={onBack}
          onClose={vi.fn()}
        >
          <p>Fields</p>
        </PublishModal>,
      );
      fireEvent.click(screen.getByRole('button', { name: 'back' }));
      expect(onBack).toHaveBeenCalledTimes(1);
    });
  });
});
