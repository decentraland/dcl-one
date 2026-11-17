// A record of the central directory. In a zip64 archive, the sizes and the offset come from the
// zip64 field and can be larger than 4 GiB.
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
