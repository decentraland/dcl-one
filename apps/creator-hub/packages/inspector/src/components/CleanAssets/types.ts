export type FileSize = {
  path: string;
  size: number; // in bytes
};

export type AssetFile = {
  path: string;
  size: number; // in bytes
  unused: boolean;
  /** The scene's code names this file. */
  usedByCode?: boolean;
};

export type ScanResult = {
  assets: AssetFile[];
  /** The scene has code and all of it was read, so the files it names are marked as used. */
  codeScanned: boolean;
};

export type Props = {
  isOpen: boolean;
  assets: AssetFile[];
  /** Whether the scan read the scene's code; the note about code only holds when it did. */
  codeScanned: boolean;
  isScanning: boolean;
  selectedAssets: Set<string>;
  onClose: () => void;
  onScan: () => void;
  onSelect: (asset: string) => void;
  onSelectAll: (selectAll: boolean) => void;
};
