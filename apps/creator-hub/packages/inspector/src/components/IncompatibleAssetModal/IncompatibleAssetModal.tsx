import React, { useCallback, useState } from 'react';
import { IoClose } from 'react-icons/io5';
import { Modal } from '../Modal';
import { Button } from '../Button';
import type { IncompatibleComponent } from '../../lib/sdk/operations/add-asset/compatibility';

import './IncompatibleAssetModal.css';

const REASON_LABEL: Record<IncompatibleComponent['reason'], string> = {
  missing: ' — not available',
  'outdated-definition': ' — outdated definition',
  'unsupported-script-param': ' — needs a newer Creator Hub',
  'sdk-too-old': ' or newer — this scene has an older version',
};

const isScriptParam = ({ reason }: IncompatibleComponent) => reason === 'unsupported-script-param';
const isSdkTooOld = ({ reason }: IncompatibleComponent) => reason === 'sdk-too-old';

type SdkUpdate = 'idle' | 'updating' | 'updated' | 'failed';

interface Props {
  assetName: string;
  incompatibleComponents: IncompatibleComponent[];
  onClose: () => void;
  /**
   * Updates the scene's SDK, resolving to whether it now satisfies the asset. Offered as a button
   * when the scene's SDK is the only thing in the way. Absent when no host can run the update.
   */
  onUpdateSdk?: () => Promise<boolean>;
}

const IncompatibleAssetModal: React.FC<Props> = ({
  assetName,
  incompatibleComponents,
  onClose,
  onUpdateSdk,
}) => {
  const [sdkUpdate, setSdkUpdate] = useState<SdkUpdate>('idle');
  const needsEditorUpdate = incompatibleComponents.some(isScriptParam);
  const needsNewerSdk = incompatibleComponents.some(isSdkTooOld);
  const needsDependencyUpdate = incompatibleComponents.some(
    c => !isScriptParam(c) && !isSdkTooOld(c),
  );
  // Updating the SDK doesn't help while something else also blocks the asset.
  const canUpdateSdk = !!onUpdateSdk && incompatibleComponents.every(isSdkTooOld);
  const updating = sdkUpdate === 'updating';

  const handleUpdateSdk = useCallback(async () => {
    if (!onUpdateSdk) return;
    setSdkUpdate('updating');
    setSdkUpdate((await onUpdateSdk()) ? 'updated' : 'failed');
  }, [onUpdateSdk]);

  return (
    <Modal
      isOpen
      onRequestClose={updating ? undefined : onClose}
      className="IncompatibleAssetModal"
    >
      <header className="IncompatibleAssetModalHeader">
        <h2>{sdkUpdate === 'updated' ? 'SDK Updated' : 'Dependency Update Required'}</h2>
        <button
          className="CloseButton"
          aria-label="Close"
          onClick={onClose}
          disabled={updating}
        >
          <IoClose size={24} />
        </button>
      </header>

      {sdkUpdate === 'updated' ? (
        <div className="IncompatibleAssetModalBody">
          <p>
            The scene's SDK is up to date. Add <strong>"{assetName}"</strong> to the scene again.
          </p>
        </div>
      ) : (
        <div className="IncompatibleAssetModalBody">
          {needsNewerSdk && (
            <p>
              <strong>"{assetName}"</strong> needs a newer SDK than this scene has. Update the
              scene's SDK to use this asset.
            </p>
          )}
          {needsDependencyUpdate && (
            <p>
              <strong>"{assetName}"</strong> requires components that are not compatible with your
              current SDK version. Update your dependencies to use this asset.
            </p>
          )}
          {needsEditorUpdate && (
            <p>
              <strong>"{assetName}"</strong> has settings this version of Creator Hub can't edit.
              Update Creator Hub to use this asset.
            </p>
          )}
          <ul className="IncompatibleComponentList">
            {incompatibleComponents.map(({ name, reason }) => (
              <li key={name}>
                <code>{name}</code>
                <span className="IncompatibleReason">{REASON_LABEL[reason]}</span>
              </li>
            ))}
          </ul>
          {sdkUpdate === 'failed' && (
            <p
              className="IncompatibleAssetModalError"
              role="alert"
            >
              Couldn't update the SDK automatically. Update <strong>@dcl/sdk</strong> in your scene
              and try again.
            </p>
          )}
        </div>
      )}

      <div className="IncompatibleAssetModalFooter">
        <Button
          onClick={onClose}
          disabled={updating}
        >
          Close
        </Button>
        {canUpdateSdk && sdkUpdate !== 'updated' && (
          <Button
            type="blue"
            onClick={handleUpdateSdk}
            disabled={updating}
          >
            {updating ? 'Updating…' : 'Update SDK'}
          </Button>
        )}
      </div>
    </Modal>
  );
};

export default React.memo(IncompatibleAssetModal);
