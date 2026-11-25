import { sdk } from '@dcl/schemas'
import type { Stats } from 'fs'
import path from 'path'
import { WebSocket } from 'ws'
import chokidar from 'chokidar'
import type { Ignore } from 'ignore'
import ignore from 'ignore'
import { getDCLIgnorePatterns } from '../../../logic/dcl-ignore'
import type { PreviewComponents } from '../types'
import { sceneUpdateClients } from './routes'
import type { ProjectUnion } from '../../../logic/project-validations'
import { b64HashingFunction } from '../../../logic/project-files'
import {
  WsSceneMessage,
  UpdateModelType
} from '@dcl/protocol/out-js/decentraland/sdk/development/local_development.gen'
import { debounce } from '../../../logic/debounce'

/**
 * This function gets file modification events and sends them to all the connected
 * websockets, it is used to hot-reload assets of the scene.
 */
export async function wireFileWatcherToWebSockets(
  components: Pick<PreviewComponents, 'fs' | 'ws' | 'logger'>,
  projectRoot: string,
  projectKind: ProjectUnion['kind']
) {
  const root = path.resolve(projectRoot)
  // The .dclignore patterns, read as the deployment reads them (project-files.ts).
  const ig = ignore().add(await getDCLIgnorePatterns(components, projectRoot))
  const sceneId = b64HashingFunction(projectRoot)

  chokidar
    .watch(root, {
      atomic: false,
      ignored: dclIgnored(root, ig),
      ignoreInitial: false,
      cwd: projectRoot
    })
    .on('unlink', (file: string) => {
      removeModel(sceneId, file)
    })
    .on(
      'all',
      debounce(async (_, file) => {
        updateScene(sceneId, file)
        // Legacy JSON protocol: still the only one Bevy and Godot explorers understand.
        // Remove once both consume the protobuf WsSceneMessage.
        __LEGACY__updateScene(projectRoot, sceneUpdateClients, projectKind)
      }, 800)
    )
}

/** The watcher's `ignored` test: true for a path in the project that `.dclignore` names. */
export function dclIgnored(root: string, ig: Ignore) {
  return (file: string, stats?: Stats) => {
    const relative = path.relative(root, path.resolve(root, file))
    if (!relative || relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) {
      return false
    }
    const name = relative.split(path.sep).join('/')
    return ig.ignores(stats?.isDirectory() ? name + '/' : name)
  }
}

function isGLTFModel(file: string) {
  if (!file) return false
  return file.toLowerCase().endsWith('.glb') || file.toLowerCase().endsWith('.gltf')
}

function updateScene(sceneId: string, file: string) {
  let message: WsSceneMessage['message']
  if (isGLTFModel(file)) {
    message = {
      $case: 'updateModel',
      updateModel: { hash: b64HashingFunction(file), sceneId, src: file, type: UpdateModelType.UMT_CHANGE }
    }
  } else {
    message = {
      $case: 'updateScene',
      updateScene: { sceneId }
    }
  }
  sendSceneMessage({ message })
}

function removeModel(sceneId: string, file: string) {
  if (isGLTFModel(file)) {
    const sceneMessage: WsSceneMessage = {
      message: {
        $case: 'updateModel',
        updateModel: { sceneId, src: file, hash: b64HashingFunction(file), type: UpdateModelType.UMT_REMOVE }
      }
    }

    sendSceneMessage(sceneMessage)
  }
}

function sendSceneMessage(sceneMessage: WsSceneMessage) {
  const message = WsSceneMessage.encode(sceneMessage).finish()
  for (const client of sceneUpdateClients) {
    if (client.readyState === WebSocket.OPEN) {
      client.send(message, { binary: true })
    }
  }
}

/**
 * @deprecated legacy JSON protocol, consumed by Bevy and Godot explorers
 */
export function __LEGACY__updateScene(dir: string, clients: Set<WebSocket>, projectKind: ProjectUnion['kind']): void {
  for (const client of clients) {
    if (client.readyState === WebSocket.OPEN) {
      const message: sdk.SceneUpdate = {
        type: sdk.SCENE_UPDATE,
        payload: { sceneId: b64HashingFunction(dir), sceneType: projectKind }
      }

      client.send(sdk.UPDATE, { binary: false })
      client.send(JSON.stringify(message), { binary: false })
    }
  }
}
