import { useCallback } from 'react';
import { useStore } from 'react-redux';

import type { PreviewOptions } from '/shared/types/settings';

import { actions as editorActions, type PublishSceneOptions } from '/@/modules/store/editor';
import { actions as workspaceActions } from '/@/modules/store/workspace';
import type { Method, Params } from '/@/modules/rpc/storage';
import type { RPCInfo } from '/@/modules/rpc';
import { bufferToScene } from '/@/modules/buffer';
import { useInspector } from '/@/hooks/useInspector';
import { useAuth } from '/@/hooks/useAuth';
import { stripBase64ImagePrefix } from '/@/modules/image';
import { editor as editorApi } from '#preload';
import { type AppState, useDispatch, useSelector } from '#store';

export const useEditor = () => {
  const dispatch = useDispatch();
  const store = useStore<AppState>();
  const { chainId, wallet } = useAuth();
  const { generateThumbnail } = useInspector();
  const editor = useSelector(state => state.editor);
  const { project } = editor;

  const startInspector = useCallback(() => {
    dispatch(editorActions.startInspector());
  }, [dispatch, editorActions.startInspector]);

  const publishScene = useCallback(
    (opts: Omit<PublishSceneOptions, 'path' | 'chainId' | 'wallet'> = {}) => {
      if (project && chainId && wallet) {
        return dispatch(
          editorActions.publishScene({ ...opts, path: project.path, chainId, wallet }),
        ).unwrap();
      }
    },
    [project, editorActions.publishScene, chainId, wallet],
  );

  const openPreview = useCallback(
    (opts: PreviewOptions) => {
      if (project) {
        return dispatch(editorActions.runScene({ path: project.path, ...opts })).unwrap();
      }
    },
    [project, editorActions.runScene],
  );

  const killPreview = useCallback(() => {
    if (project) {
      dispatch(editorActions.killPreviewScene(project.path));
    }
  }, [project, editorActions.killPreviewScene]);

  const leaveProject = useCallback(() => {
    dispatch(editorActions.leaveProject());
  }, [editorActions.leaveProject]);

  const openCode = useCallback(() => {
    if (project) {
      editorApi.openCode(project.path);
    }
  }, [editorApi.openCode, project]);

  // The inspector RPC keeps the handler it got when the iframe loaded, so it reads the open
  // project from the store: a copy captured then would undo every later change (the world and
  // deployments a publish recorded) on the next scene.json write, such as a rename.
  const updateScene = useCallback(
    async (_: RPCInfo, { path, content }: Params[Method.WRITE_FILE]) => {
      const current = store.getState().editor.project;
      if (current && path === 'scene.json') {
        const scene = bufferToScene(content);
        dispatch(
          workspaceActions.updateProject({
            ...current,
            title: scene.display?.title || current.title,
            scene: scene.scene,
          }),
        );
      }
    },
    [store],
  );

  const saveAndGetThumbnail = useCallback(async (rpcInfo: RPCInfo) => {
    const thumbnail = await generateThumbnail(rpcInfo);
    if (thumbnail) {
      const data = { path: rpcInfo.project.path, thumbnail: stripBase64ImagePrefix(thumbnail) };
      return dispatch(workspaceActions.saveAndGetThumbnail(data)).unwrap();
    }
  }, []);

  // TODO: find a proper name for this function
  const refreshProject = useCallback(
    async (rpcInfo: RPCInfo) => {
      const _ = await saveAndGetThumbnail(rpcInfo);
      dispatch(
        workspaceActions.getProject({
          path: rpcInfo.project.path,
          opts: { omitOutdatedPackages: true },
        }),
      );
    },
    [workspaceActions.getProject, project],
  );

  const getMobileQR = useCallback(
    (opts: PreviewOptions) => {
      if (project) {
        return dispatch(editorActions.getMobileQR({ path: project.path, opts })).unwrap();
      }
    },
    [project],
  );

  // Start/kill the headless Bevy realm for a project (the sdk-commands content +
  // data-layer the embedded Bevy editor engine loads from). Direct
  // request/response like openCode — no redux round-trip needed. Path is passed
  // in (not closed over `project`) so these stay referentially stable across
  // project-object updates — the realm effect keys on path and must not restart
  // the server on every scene edit.
  const startBevyRealm = useCallback(
    (path: string) => {
      return editorApi.startBevyRealm(path);
    },
    [editorApi.startBevyRealm],
  );

  const killBevyRealm = useCallback(
    (path: string) => {
      return editorApi.killBevyRealm(path);
    },
    [editorApi.killBevyRealm],
  );

  return {
    ...editor,
    startInspector,
    publishScene,
    openPreview,
    killPreview,
    leaveProject,
    openCode,
    updateScene,
    saveAndGetThumbnail,
    refreshProject,
    getMobileQR,
    startBevyRealm,
    killBevyRealm,
  };
};
