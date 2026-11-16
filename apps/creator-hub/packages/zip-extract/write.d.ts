export type ZipEntry = { name: string; store?: boolean } & (
  | { content: string | Uint8Array }
  | { file: string }
);

export type ZipOptions = {
  date?: Date;
  onEntry?: (entry: ZipEntry, written: number) => void;
  // Writes the zip64 records for each entry and for the archive, also where the plain format is
  // large enough.
  forceZip64?: boolean;
};

export function zip(
  entries: Iterable<ZipEntry> | AsyncIterable<ZipEntry>,
  options?: ZipOptions,
): AsyncGenerator<Buffer, void, undefined>;
