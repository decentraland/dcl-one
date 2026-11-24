import type { Project } from '/shared/types/projects';

type ProjectsState = {
  editor: { project?: Project };
  workspace: { projects: Project[] };
};

/**
 * The open project as the workspace holds it. A publish records its deployments and
 * `publishedAt` only on the workspace entry, so a write built from `editor.project` alone would
 * drop them (and the next publish would save the shortened deployments to disk).
 */
export function getOpenProject({ editor, workspace }: ProjectsState) {
  const open = editor.project;
  return open && (workspace.projects.find($ => $.path === open.path) ?? open);
}
