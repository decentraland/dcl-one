import { useMemo, useState } from "react";
import ContentStatus from "../../components/ContentStatus";
import { sendBridge } from "../../overlay/bridge";
import { parseParcel } from "../../data/catalyst/landListing";
import { teleportToSpawn } from "../../data/previewMode";
import { realmBounds, sceneAt, useRealmScenes } from "../../data/realmMap";
import "./realmmap.css";

const PARCEL_SIZE = 16;
const MINI_RADIUS = 6;

export function sceneHue(index: number): number {
  return (index * 67 + 190) % 360;
}

export function teleportToParcel(x: number, y: number): void {
  sendBridge("Teleport", { x: x * PARCEL_SIZE + PARCEL_SIZE / 2, z: y * PARCEL_SIZE + PARCEL_SIZE / 2 });
}

type Variant = "full" | "compact" | "mini";

/** The map of a realm that is not Genesis City: every scene it serves, drawn from its own
    content API. `mini` is the Location card's square, centred on the player; `compact` is the
    Nearby embed; `full` is the Map panel, with a legend and a card for the picked parcel. */
export default function RealmMap({ base, coords, heading, variant = "full" }: {
  base: string;
  coords?: string | null;
  heading?: number | null;
  variant?: Variant;
}) {
  const scenes = useRealmScenes(base);
  const list = scenes.data ?? [];
  const player = parseParcel(coords);
  const [picked, setPicked] = useState<[number, number] | null>(null);
  const bounds = useMemo(() => {
    if (variant === "mini") {
      const [cx, cy] = player ?? list[0]?.base ?? [0, 0];
      return { minX: cx - MINI_RADIUS, maxX: cx + MINI_RADIUS, minY: cy - MINI_RADIUS, maxY: cy + MINI_RADIUS };
    }
    const b = realmBounds(list);
    if (!b) return player ? { minX: player[0] - 2, maxX: player[0] + 2, minY: player[1] - 2, maxY: player[1] + 2 } : null;
    const pad = variant === "full" ? 2 : 1;
    const withPlayer = player ? { minX: Math.min(b.minX, player[0]), maxX: Math.max(b.maxX, player[0]), minY: Math.min(b.minY, player[1]), maxY: Math.max(b.maxY, player[1]) } : b;
    return { minX: withPlayer.minX - pad, maxX: withPlayer.maxX + pad, minY: withPlayer.minY - pad, maxY: withPlayer.maxY + pad };
  }, [list, player, variant]);
  if (!bounds) {
    return <div className={`rm rm--${variant}`} data-empty>{scenes.isPending ? <ContentStatus pending message="Reading the realm&hellip;" /> : <ContentStatus message="This realm reported no scenes." onRetry={() => { void scenes.refetch(); }} />}</div>;
  }
  const width = bounds.maxX - bounds.minX + 1;
  const height = bounds.maxY - bounds.minY + 1;
  // Parcel (x, y) -> SVG cell: x grows east, y grows north, so the y axis flips.
  const cx = (x: number) => x - bounds.minX;
  const cy = (y: number) => bounds.maxY - y;
  const pickedScene = picked ? sceneAt(list, picked[0], picked[1]) : null;
  const interactive = variant === "full";
  const onCell = (x: number, y: number) => {
    if (!interactive) return;
    setPicked((prev) => (prev && prev[0] === x && prev[1] === y ? null : [x, y]));
  };
  return (
    <div className={`rm rm--${variant}`} data-scenes={list.length}>
      <svg className="rm__svg" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio={variant === "mini" ? "xMidYMid slice" : "xMidYMid meet"} role="img" aria-label={`Realm map, ${list.length} scene${list.length === 1 ? "" : "s"}`}>
        <defs>
          <pattern id="rm-grid" width="1" height="1" patternUnits="userSpaceOnUse">
            <path d="M 1 0 L 0 0 0 1" fill="none" stroke="rgba(255,255,255,.08)" strokeWidth="0.04" />
          </pattern>
        </defs>
        <rect x="0" y="0" width={width} height={height} fill="#17131b" />
        <rect x="0" y="0" width={width} height={height} fill="url(#rm-grid)" />
        {list.map((scene, i) => (
          <g key={scene.id} className="rm__scene" data-scene={scene.id} style={{ color: `hsl(${sceneHue(i)} 55% 48%)` }}>
            {scene.parcels.map(([x, y]) => (
              <rect key={`${x},${y}`} className="rm__cell" x={cx(x) + 0.04} y={cy(y) + 0.04} width={0.92} height={0.92} fill="currentColor" onClick={() => onCell(x, y)} data-picked={picked && picked[0] === x && picked[1] === y ? "true" : undefined}>
                <title>{`${scene.title} · ${x},${y}`}</title>
              </rect>
            ))}
            {variant !== "mini" && (() => {
              // Label at the scene's top-left cell, sized to its width so a small scene stays legible
              // and a large one is not a banner; the SVG scales it with the viewport.
              const minX = Math.min(...scene.parcels.map((p) => p[0]));
              const maxX = Math.max(...scene.parcels.map((p) => p[0]));
              const maxY = Math.max(...scene.parcels.map((p) => p[1]));
              const size = Math.min(0.42, Math.max(0.22, (maxX - minX + 1) * 0.06));
              return <text className="rm__label" x={cx(minX) + 0.12} y={cy(maxY) + size + 0.1} fontSize={size} textLength={undefined}>{scene.title}</text>;
            })()}
            {scene.spawns.map((spawn) => (
              <circle key={spawn.name} className="rm__spawn" cx={cx(spawn.parcel[0]) + ((spawn.world.x - spawn.parcel[0] * PARCEL_SIZE) / PARCEL_SIZE)} cy={cy(spawn.parcel[1]) + 1 - ((spawn.world.z - spawn.parcel[1] * PARCEL_SIZE) / PARCEL_SIZE)} r={0.14}>
                <title>{`Spawn: ${spawn.name}`}</title>
              </circle>
            ))}
          </g>
        ))}
        {player && (
          <g className="rm__player" transform={`translate(${cx(player[0]) + 0.5} ${cy(player[1]) + 0.5})`} aria-label={`You at ${player[0]},${player[1]}`}>
            <circle r={0.42} />
            <path d="M0 -0.3 L0.22 0.22 L0 0.1 L-0.22 0.22 Z" transform={heading == null ? undefined : `rotate(${heading})`} />
          </g>
        )}
      </svg>
      {variant === "full" && (
        <aside className="rm__legend" aria-label="Scenes in this realm">
          {list.map((scene, i) => (
            <button key={scene.id} type="button" className="rm__legenditem" data-active={pickedScene?.id === scene.id || undefined} onClick={() => setPicked(scene.spawns.find((s) => s.isDefault)?.parcel ?? scene.spawns[0]?.parcel ?? scene.base)}>
              <i style={{ background: `hsl(${sceneHue(i)} 55% 48%)` }} aria-hidden="true" />
              <span className="rm__legendtitle">{scene.title}</span>
              <small>{scene.parcels.length} parcel{scene.parcels.length === 1 ? "" : "s"}</small>
            </button>
          ))}
          {scenes.isError && <ContentStatus message="Couldn't read the realm's scenes." onRetry={() => { void scenes.refetch(); }} />}
        </aside>
      )}
      {variant === "full" && picked && (
        <div className="rm__card" role="dialog" aria-label={`Parcel ${picked[0]},${picked[1]}`}>
          <strong>{pickedScene?.title ?? "Empty parcel"}</strong>
          <span>{picked[0]},{picked[1]}{pickedScene ? ` · ${pickedScene.parcels.length} parcels` : ""}</span>
          <div className="rm__cardactions">
            <button type="button" className="rm__jump" onClick={() => teleportToParcel(picked[0], picked[1])}>Jump here</button>
            {pickedScene?.spawns.slice(0, 3).map((spawn) => (
              <button key={spawn.name} type="button" className="rm__spawnbtn" onClick={() => teleportToSpawn(spawn)}>{spawn.name}</button>
            ))}
            <button type="button" className="rm__close" aria-label="Close" onClick={() => setPicked(null)}>&#xD7;</button>
          </div>
        </div>
      )}
    </div>
  );
}
