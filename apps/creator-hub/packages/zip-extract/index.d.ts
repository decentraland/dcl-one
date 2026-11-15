export type Entry = {
  fileName: string;
  flags: number;
  method: number;
  crc32: number;
  compressedSize: number;
  uncompressedSize: number;
  mode: number;
  localOffset: number;
};

declare function extract(
  zipPath: string,
  options: { dir: string; onEntry?: (entry: Entry) => void },
): Promise<void>;

export default extract;
