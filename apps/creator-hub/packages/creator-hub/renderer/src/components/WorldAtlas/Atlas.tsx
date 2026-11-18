import { useCallback, useLayoutEffect, useMemo, useRef, useState } from 'react';
import LandMap from '@ui/shop/land/LandMap';
import type { MapLayer, MapViewport } from '@ui/shop/land/mapView';
import type { Tiles } from '@ui/shop/land/tiles';

import type { AtlasTileProps } from '/@/modules/store/land';

import styles from './Atlas.module.css';

export type AtlasLayer = MapLayer;
export type AtlasViewport = MapViewport;

export type AtlasProps = {
  /** The Genesis City tiles. Absent or empty: the map shows the empty grid only. */
  tiles?: Record<string, AtlasTileProps>;
  layers?: AtlasLayer[];
  x?: number;
  y?: number;
  height?: number;
  isDraggable?: boolean;
  withZoomControls?: boolean;
  label?: string;
  onHover?: (x: number, y: number) => void;
  onClick?: (x: number, y: number) => void;
  onMouseDown?: (x: number, y: number) => void;
  onMouseUp?: (x: number, y: number) => void;
  onChange?: (viewport: AtlasViewport) => void;
};

// The ui3 map has 340 parcels on a side and its zoom 1 fits them in the smaller side of the map.
const GRID_PARCELS = 340;
const PARCEL_PX = 14;
const MIN_PARCEL_PX = 7;
const MAX_PARCEL_PX = 40;
const EMPTY_ODD = '#110E13';
const EMPTY_EVEN = '#0D0B0E';

const NO_LAYERS: AtlasLayer[] = [];
const ZOOM_BUTTONS = { inLabel: 'Add zoom', outLabel: 'Decrease zoom', outText: '-' };

function toMapTiles(tiles: Record<string, AtlasTileProps>): Tiles {
  const out: Tiles = {};
  for (const [id, tile] of Object.entries(tiles)) {
    out[id] = {
      id,
      x: tile.x,
      y: tile.y,
      kind: tile.type,
      top: !!tile.top,
      left: !!tile.left,
      topLeft: !!tile.topLeft,
      owner: tile.owner ? tile.owner.toLowerCase() : null,
      name: tile.name || null,
      estateId: tile.estateId || null,
      price: null,
      rentalPricePerDay: null,
    };
  }
  return out;
}

/** The parcel map of Creator Hub: the shared ui3 LAND map behind the props of the former Atlas. */
export function Atlas({
  tiles,
  layers = NO_LAYERS,
  x,
  y,
  height = 480,
  isDraggable = true,
  withZoomControls = false,
  label,
  onHover,
  onClick,
  onMouseDown,
  onMouseUp,
  onChange,
}: AtlasProps) {
  const mapTiles = useMemo(() => toMapTiles(tiles ?? {}), [tiles]);

  const paintLayers = useMemo(() => {
    const emptyLayer: AtlasLayer = (px, py) =>
      mapTiles[`${px},${py}`] ? null : { color: (px + py) % 2 === 0 ? EMPTY_ODD : EMPTY_EVEN };
    return [emptyLayer, ...layers];
  }, [mapTiles, layers]);

  // Parcel 0,0 is the middle of the map when the caller gives no place, as in the former Atlas.
  const center = useMemo(() => ({ x: x ?? 0, y: y ?? 0 }), [x, y]);

  // The map reads its first zoom once: it opens after the width of the frame is known.
  const frame = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState<number | null>(null);
  useLayoutEffect(() => {
    const el = frame.current;
    if (!el) return;
    const measure = () => setWidth(el.getBoundingClientRect().width);
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  // A frame with no width yet (no layout) uses the height.
  const side = width ? Math.min(width, height) : height;

  const zoomOf = (parcelPx: number) => (parcelPx * GRID_PARCELS) / side;

  // One press changes a parcel by a tenth of its start size, in each direction.
  const zoomStep = useCallback(
    (zoom: number, direction: 1 | -1) =>
      zoom + (direction * ((PARCEL_PX * GRID_PARCELS) / side)) / 10,
    [side],
  );

  const pointer = isDraggable ? '' : onClick ? styles.clickable : styles.fixed;

  return (
    <div
      ref={frame}
      className={styles.frame}
      style={{ height }}
    >
      {width === null ? null : (
        <LandMap
          className={`${styles.map} ${pointer}`}
          tiles={mapTiles}
          surface="tiles"
          paintLayers={paintLayers}
          center={center}
          initialZoom={zoomOf(PARCEL_PX)}
          minZoom={zoomOf(MIN_PARCEL_PX)}
          maxZoom={zoomOf(MAX_PARCEL_PX)}
          draggable={isDraggable}
          dragWithAnyButton
          wheelZoom="always"
          withZoomControls={withZoomControls}
          zoomButtons={ZOOM_BUTTONS}
          zoomStep={zoomStep}
          paintInTask
          overscan={1}
          withPopup={false}
          label={label}
          onParcelHover={onHover}
          onParcelClick={onClick}
          onParcelDown={onMouseDown}
          onParcelUp={onMouseUp}
          onViewChange={onChange}
        />
      )}
    </div>
  );
}
