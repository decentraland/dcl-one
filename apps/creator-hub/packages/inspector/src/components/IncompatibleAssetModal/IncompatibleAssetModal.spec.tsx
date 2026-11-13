import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import type { IncompatibleComponent } from '../../lib/sdk/operations/add-asset/compatibility';
import IncompatibleAssetModal from './IncompatibleAssetModal';

const SDK_TOO_OLD: IncompatibleComponent = { name: '@dcl/sdk 7.32.0', reason: 'sdk-too-old' };
const UNKNOWN_PARAM: IncompatibleComponent = {
  name: 'skybox-controller.ts: fogColor',
  reason: 'unsupported-script-param',
};

describe('IncompatibleAssetModal', () => {
  let onUpdateSdk: ReturnType<typeof vi.fn<[], Promise<boolean>>>;

  const renderModal = (
    incompatibleComponents: IncompatibleComponent[],
    update: (() => Promise<boolean>) | undefined = onUpdateSdk,
  ) =>
    render(
      <IncompatibleAssetModal
        assetName="Skybox"
        incompatibleComponents={incompatibleComponents}
        onClose={() => {}}
        onUpdateSdk={update}
      />,
    );
  const updateButton = () => screen.queryByRole('button', { name: 'Update SDK' });

  beforeEach(() => {
    onUpdateSdk = vi.fn<[], Promise<boolean>>();
  });

  describe('when the scene has an older SDK than the asset needs', () => {
    it('should name the SDK version the asset needs', () => {
      renderModal([SDK_TOO_OLD]);
      expect(screen.getByText('@dcl/sdk 7.32.0')).toBeTruthy();
    });

    it('should offer to update the SDK', () => {
      renderModal([SDK_TOO_OLD]);
      expect(updateButton()).toBeTruthy();
    });

    describe('and the update succeeds', () => {
      beforeEach(() => {
        onUpdateSdk.mockResolvedValue(true);
        renderModal([SDK_TOO_OLD]);
        fireEvent.click(updateButton()!);
      });

      it('should ask to add the asset again', async () => {
        await waitFor(() => expect(screen.getByText(/to the scene again/)).toBeTruthy());
      });

      it('should stop offering the update', async () => {
        await waitFor(() => expect(updateButton()).toBeNull());
      });
    });

    describe('and the update fails', () => {
      beforeEach(() => {
        onUpdateSdk.mockResolvedValue(false);
        renderModal([SDK_TOO_OLD]);
        fireEvent.click(updateButton()!);
      });

      it('should say so and keep offering the update', async () => {
        await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
        expect(updateButton()).toBeTruthy();
      });
    });
  });

  describe('when no host can update the SDK', () => {
    it('should not offer to update it', () => {
      render(
        <IncompatibleAssetModal
          assetName="Skybox"
          incompatibleComponents={[SDK_TOO_OLD]}
          onClose={() => {}}
        />,
      );
      expect(updateButton()).toBeNull();
    });
  });

  describe('when the asset is also blocked by something an SDK update does not fix', () => {
    it('should not offer to update the SDK', () => {
      renderModal([SDK_TOO_OLD, UNKNOWN_PARAM]);
      expect(updateButton()).toBeNull();
    });
  });
});
