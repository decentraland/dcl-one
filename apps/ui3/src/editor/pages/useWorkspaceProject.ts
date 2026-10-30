import { useEffect, useRef, useState, type RefObject } from "react";
import type { EditorBus } from "../editor-bus";
import { folderProject } from "../folder-project";
import { registerProjectContents } from "../project-cache";
import type { DeWorkspaceCode } from "../types";

export function useWorkspaceProject(code: DeWorkspaceCode | null, busRef: RefObject<EditorBus | null>) {
  const [folderFiles, setFolderFiles] = useState<NonNullable<DeWorkspaceCode["project"]> | null>(null);
  const folderPromise = useRef<Promise<NonNullable<DeWorkspaceCode["project"]> | null> | null>(null);
  useEffect(() => {
    let current = true;
    setFolderFiles(null);
    const pending = !code?.project && code?.getDir
      ? code.getDir().then(directory => directory ? folderProject(directory) : null)
      : Promise.resolve(null);
    folderPromise.current = pending;
    void pending.then(project => { if (current) setFolderFiles(project); }).catch(() => {});
    return () => { current = false; };
  }, [code]);
  const projectFiles = code?.project ?? folderFiles;
  const prepareScene = code?.getDir || code?.project?.assets?.preparePreview ? async () => {
    const project = code?.project ?? await folderPromise.current;
    await registerProjectContents(busRef, project?.assets);
  } : undefined;
  return { projectFiles, prepareScene };
}
