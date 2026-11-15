export type ZipEntry = { name: string; store?: boolean } & (
  | { content: string | Uint8Array }
  | { file: string }
);

export type ZipOptions = {
  date?: Date;
  onEntry?: (entry: ZipEntry, written: number) => void;
};

export function zip(
  entries: Iterable<ZipEntry> | AsyncIterable<ZipEntry>,
  options?: ZipOptions,
): AsyncGenerator<Buffer, void, undefined>;
