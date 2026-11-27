import type { ILoggerComponent, IConfigComponent } from '@well-known-components/interfaces'
import { spawn } from 'child_process'
import type { IProcessSpawnerComponent } from '../logic/exec'
import { createProcessSpawnerComponent } from '../logic/exec'
import type { IAnalyticsComponent } from './analytics'
import { createAnalyticsComponent } from './analytics'
import { createConfigComponent } from './config'
import type { IFetchComponent } from './fetch'
import { createFetchComponent } from './fetch'
import type { IFileSystemComponent } from './fs'
import { createFsComponent } from './fs'
import { createStderrCliLogger } from './log'

export type CliComponents = {
  fs: IFileSystemComponent
  fetch: IFetchComponent
  logger: ILoggerComponent.ILogger
  analytics: IAnalyticsComponent
  spawner: IProcessSpawnerComponent
  config: IConfigComponent
}

export async function initComponents(): Promise<CliComponents> {
  const fs = createFsComponent()
  const config = await createConfigComponent({ fs })
  const logger = createStderrCliLogger()
  const spawner = createProcessSpawnerComponent(spawn)

  return {
    fs,
    fetch: createFetchComponent(),
    logger,
    config,
    spawner,
    analytics: await createAnalyticsComponent({ config, logger, fs })
  }
}
