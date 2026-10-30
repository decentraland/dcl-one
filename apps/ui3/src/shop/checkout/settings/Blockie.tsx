// from: ui@8a63476:src/components/Blockie/Blockie.{tsx,css} (decentraland-ui's Blockie: the fixed
//       #e449c2/#3149de/#e449c2 palette, 6px corners) over ethereum-blockies@0.1.1:blockies.js (the
//       xorshift pattern, MIT). Copyright 2020 Metaverse Holdings Ltd. (decentraland-ui), Apache-2.0.
function pattern(seed: string, size: number): number[] {
  const rs = [0, 0, 0, 0];
  for (let i = 0; i < seed.length; i++) rs[i % 4] = ((rs[i % 4]! << 5) - rs[i % 4]! + seed.charCodeAt(i)) | 0;
  const rand = () => {
    const t = rs[0]! ^ (rs[0]! << 11);
    rs[0] = rs[1]!;
    rs[1] = rs[2]!;
    rs[2] = rs[3]!;
    rs[3] = rs[3]! ^ (rs[3]! >> 19) ^ t ^ (t >> 8);
    return (rs[3]! >>> 0) / ((1 << 31) >>> 0);
  };
  const data: number[] = [];
  const dataWidth = Math.ceil(size / 2);
  const mirrorWidth = size - dataWidth;
  for (let y = 0; y < size; y++) {
    const row: number[] = [];
    for (let x = 0; x < dataWidth; x++) row[x] = Math.floor(rand() * 2.3);
    data.push(...row, ...row.slice(0, mirrorWidth).reverse());
  }
  return data;
}

export default function Blockie({ seed, size = 6, scale = 12, className }: { seed: string; size?: number; scale?: number; className?: string }) {
  const cells = pattern(seed.toLowerCase(), size);
  const px = size * scale;
  return (
    <svg className={className} width={px} height={px} viewBox={`0 0 ${size} ${size}`} shapeRendering="crispEdges" aria-hidden="true">
      <rect width={size} height={size} fill="#3149de" />
      {cells.map((v, i) => (v ? <rect key={i} x={i % size} y={Math.floor(i / size)} width={1} height={1} fill="#e449c2" /> : null))}
    </svg>
  );
}
