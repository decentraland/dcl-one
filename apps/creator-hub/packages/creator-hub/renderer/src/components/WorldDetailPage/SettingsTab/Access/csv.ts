const ADDRESS = /^0x[a-fA-F0-9]{40}$/;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export type CsvResult = {
  fileName: string;
  addresses: string[];
  communityIds: string[];
  invalidLines: string[];
};

/** A CSV of invites, one wallet address or community id per line; anything else is an invalid line. */
export function parseCsv(fileName: string, content: string): CsvResult {
  const addresses = new Set<string>();
  const communityIds = new Set<string>();
  const invalidLines: string[] = [];
  for (const line of content.split(/[\r\n]+/).map(text => text.trim())) {
    if (!line) continue;
    if (ADDRESS.test(line)) addresses.add(line.toLowerCase());
    else if (UUID.test(line)) communityIds.add(line);
    else invalidLines.push(line);
  }
  return { fileName, addresses: [...addresses], communityIds: [...communityIds], invalidLines };
}
