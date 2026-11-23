import type { Project } from '/shared/types/projects';

/**
 * The world a project publishes to.
 *
 * scene.json's `worldConfiguration` names it, but the data layer bundled in an older
 * sdk-commands (the Bevy editor's) writes back the copy it loaded over the whole file, which
 * drops the world a publish just added. The latest recorded publish keeps it: a world when the
 * scene last went to a world, none when it last went to LAND.
 */
export function getProjectWorldName(project: Pick<Project, 'worldConfiguration' | 'info'>) {
  if (project.worldConfiguration?.name) return project.worldConfiguration.name;
  const records = Object.values(project.info?.deployments ?? {});
  const latest = records.reduce<(typeof records)[number] | undefined>(
    (last, record) => (!last || record.publishedAt > last.publishedAt ? record : last),
    undefined,
  );
  return latest?.worldName || undefined;
}
