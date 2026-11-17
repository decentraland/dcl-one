import React, { useCallback, useMemo } from 'react';

import { useAssetOptions } from '../../../../hooks/useAssetOptions';
import ImportAsset from '../../../ImportAsset';
import type { TreeNode } from '../../../ProjectAssetExplorer/ProjectView';
import { isAssetNode } from '../../../ProjectAssetExplorer/utils';
import { ACCEPTED_FILE_TYPES } from '../../../ui/FileUploadField/types';
import type { ScriptFileFormat } from '../types';

type Props = {
  label: React.ReactNode;
  value: string;
  format: ScriptFileFormat;
  // A warning from the inspector, e.g. that the stored file is no longer in the scene.
  error?: string;
  disabled?: boolean;
  onChange: (path: string) => void;
};

const hasExtension = (path: string, extensions: string[]) =>
  extensions.some(extension => path.toLowerCase().endsWith(extension));

// Picking it stores '', the value the script reads as "no file" (the Skybox's regular sky).
const NONE_OPTION = { label: 'None', value: '' };

// A string param tagged `@format image` (or audio, video, model): picks a scene file of that kind
// from a dropdown, the file tree or the disk, as the Material texture and AudioSource fields do.
export function FileParamField({ label, value, format, error, disabled, onChange }: Props) {
  const accept = ACCEPTED_FILE_TYPES[format];
  const assetOptions = useAssetOptions(accept);
  const options = useMemo(() => [NONE_OPTION, ...assetOptions], [assetOptions]);
  const isValidFile = useCallback(
    (node: TreeNode) => isAssetNode(node) && hasExtension(node.name, accept),
    [accept],
  );

  return (
    <ImportAsset
      mode="field"
      label={label}
      value={value}
      disabled={disabled}
      accept={accept}
      options={options}
      isValidFile={isValidFile}
      error={value && !hasExtension(value, accept) ? 'Invalid file type' : error}
      onChange={e => onChange(e.target.value)}
    />
  );
}
