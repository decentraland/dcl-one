import { useEffect, useState } from "react";
import { Link } from "react-router";
import { loadWorldScenes, sceneTravelHref, type WorldScene } from "@/data/lib/catalyst/places/world-scenes";
import styles from "./WorldScenes.module.css";

type WorldScenesProps = { worldName: string; selectedId: string; playBase?: string; onVisit?: (placeId: string, launchUrl: string) => void };

export default function WorldScenes(props: WorldScenesProps) {
  return <WorldScenePages key={props.worldName.toLowerCase()} {...props} />;
}

function WorldScenePages({ worldName, selectedId, playBase, onVisit }: WorldScenesProps) {
  const [page, setPage] = useState(0);
  const [version, setVersion] = useState(0);
  const [read, setRead] = useState<{ key: string; scenes: WorldScene[]; total: number; error?: string } | null>(null);
  const key = JSON.stringify([worldName, page, version]);
  useEffect(() => {
    const controller = new AbortController();
    loadWorldScenes(worldName, page * 20, { signal: controller.signal }).then(
      (result) => { if (!controller.signal.aborted) setRead({ key, ...result }); },
      () => { if (!controller.signal.aborted) setRead({ key, scenes: [], total: 0, error: "Scenes are temporarily unavailable." }); },
    );
    return () => controller.abort();
  }, [worldName, page, key]);
  const current = read?.key === key ? read : null;
  return <section className={styles.root} aria-label="World scenes">
    <h3 className={styles.title}>Scenes in this world</h3>
    {!current ? <p role="status">Loading scenes…</p> : current.error ? <div><p role="alert">{current.error}</p><button className={styles.button} onClick={() => setVersion((previous) => previous + 1)}>Retry scenes</button></div> : <>
      <ul className={styles.list}>{current.scenes.map((scene) => {
        const href = sceneTravelHref(worldName, scene.base_position, playBase);
        return <li className={styles.scene} key={scene.id}>
        <Link className={styles.detailLink} to={`/places/${encodeURIComponent(scene.id)}`} aria-current={selectedId === scene.id ? "page" : undefined}>{scene.title || scene.base_position}</Link>
        <span>{scene.base_position}</span>
        <a className={styles.travelLink} href={href} onClick={() => onVisit?.(scene.id, href)}>Visit {scene.title || scene.base_position}</a>
      </li>; })}</ul>
      {current.scenes.length === 0 && <p>No published scenes are listed.</p>}
      {(page > 0 || (page + 1) * 20 < current.total) && <nav className={styles.pager} aria-label="World scene pages">
        <button className={styles.button} disabled={page === 0} onClick={() => setPage((previous) => previous - 1)}>Previous scenes</button>
        <span>Page {page + 1}</span>
        <button className={styles.button} disabled={(page + 1) * 20 >= current.total} onClick={() => setPage((previous) => previous + 1)}>Next scenes</button>
      </nav>}
    </>}
  </section>;
}
