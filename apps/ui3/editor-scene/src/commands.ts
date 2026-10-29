import { type Snapshot } from './state'

export type RawConsole = (cmd: string, args?: string[]) => Promise<string>

export function makeCommands(raw: RawConsole) {
  const parse = async <T>(cmd: string, args?: string[]): Promise<T> =>
    JSON.parse(await raw(cmd, args)) as T

  return {
    crdtSnapshot: (): Promise<Snapshot> => parse<Snapshot>('crdt_snapshot'),
    crdtInitial: (): Promise<Snapshot> => parse<Snapshot>('crdt_initial'),
    registerContent: (contentsB64: string): Promise<number> => parse<number>('register_content', [contentsB64]),
    componentNames: (): Promise<string[]> => parse<string[]>('component_names'),
    componentSchema: (name: string): Promise<string> => raw('component_schema', [name]),
    componentDefault: (name: string): Promise<string> => raw('component_default', [name]),
    sceneStats: (): Promise<string> => raw('scene_stats'),
    sceneLogs: (count: number): Promise<string> => raw('scene_logs', [String(count)]),

    setComponent: (entityId: string, name: string, json: string): Promise<string> =>
      raw('set_component', [entityId, name, json]),
    setComponentRaw: (
      entityId: string,
      componentId: number,
      timestamp: number,
      base64: string
    ): Promise<string> =>
      raw('set_component_raw', [entityId, String(componentId), String(timestamp), base64]),
    deleteComponent: (entityId: string, name: string): Promise<string> =>
      raw('delete_component', [entityId, name]),
    deleteEntity: (id: string, recursive: boolean): Promise<string> =>
      raw('delete_entity', recursive ? [id, '-r'] : [id]),
    newEntity: async (componentId: number, base64: string, count: number, explicitIds?: number[]): Promise<number[]> => {
      const ids = await parse<unknown>('new_entity', [String(componentId), base64, String(count), ...(explicitIds ? ['--ids', explicitIds.join(',')] : [])])
      return Array.isArray(ids) ? ids.filter((n): n is number => typeof n === 'number') : []
    },

    initAsset: (
      assetId: string,
      contentsB64: string
    ): Promise<{
      baseDir: string
      composite: unknown
      hashes: Record<string, string>
      written: string[]
      errors: string[]
    }> => parse('init_asset', [assetId, contentsB64]),


    setScene: (hash: string): Promise<string> => raw('set_scene', [hash]),
    reload: (hash?: string): Promise<string> => raw('reload', hash === undefined ? [] : [hash]),
    freezeScene: (): Promise<string> => raw('freeze_scene'),
    unfreezeScene: (): Promise<string> => raw('unfreeze_scene'),
    tickScene: (count: number): Promise<string> => raw('tick_scene', [String(count)]),

    freezeView: (): Promise<string> => raw('freeze_view', ['on']),
    unfreezeView: (): Promise<string> => raw('freeze_view', ['off']),

    highlight: (ids: string[]): Promise<string> => raw('highlight', ids),
    setCameraProjection: (mode: 'toggle' | 'ortho' | 'perspective', scale: number): Promise<string> =>
      raw('set_camera_projection', [mode, String(scale)])
  }
}

export type Commands = ReturnType<typeof makeCommands>
