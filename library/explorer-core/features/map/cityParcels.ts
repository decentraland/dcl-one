// The parcels of Genesis City. The city is not square: two strips of extra parcels sit along its
// north and east edges.

interface ParcelRect {
  xMin: number
  yMin: number
  xMax: number
  yMax: number
}

const CITY_PARCEL_RECTS: ParcelRect[] = [
  { xMin: -150, yMin: -150, xMax: 150, yMax: 150 },
  { xMin: 62, yMin: 151, xMax: 162, yMax: 158 },
  { xMin: 151, yMin: 59, xMax: 163, yMax: 150 }
]

/** Whether Genesis City has this parcel. */
export function isCityParcel(p: { x: number; y: number }): boolean {
  return CITY_PARCEL_RECTS.some((r) => p.x >= r.xMin && p.x <= r.xMax && p.y >= r.yMin && p.y <= r.yMax)
}
