import { createAnalyticsComponent } from '../../../packages/@dcl/sdk-commands/src/components/analytics'
import { createFsComponent } from '../../../packages/@dcl/sdk-commands/src/components/fs'
import { createStderrCliLogger } from '../../../packages/@dcl/sdk-commands/src/components/log'
import { createRecordConfigComponent } from '../../../packages/@dcl/sdk-commands/node_modules/@well-known-components/env-config-provider'
import { homedir } from 'os'
import path from 'path'
import { getInstalledPackageVersion, isCI } from '../../../packages/@dcl/sdk-commands/src/logic/config'
import { requireStringConfig } from '../../../packages/@dcl/sdk-commands/src/components/config'

afterEach(() => {
  jest.clearAllMocks()
  jest.restoreAllMocks()
})

describe('Analytics Component', () => {
  test('should create .dclrc file', async () => {
    const fs = createFsComponent()
    const config = createRecordConfigComponent({
      /* empty config files */
    })
    const logger = createStderrCliLogger()
    const appendSpy = jest.spyOn(fs, 'appendFile').mockImplementation(async () => {})

    await createAnalyticsComponent({ config, logger, fs })

    // should have written the DCL_ANON_ID to the ~/.dclrc
    expect(appendSpy).toHaveBeenCalledWith(
      path.join(homedir(), '.dclrc'),
      expect.stringMatching(/\n# .+\nDCL_ANON_ID=.+/m)
    )
  })

  test('should not create .dclrc file if DCL_ANON_ID is provided', async () => {
    const fs = createFsComponent()
    const config = createRecordConfigComponent({
      DCL_ANON_ID: 'fb3f84b2-4ddc-4a7e-96bf-1e8992c294dd'
    })
    const logger = createStderrCliLogger()
    const appendSpy = jest.spyOn(fs, 'appendFile').mockImplementation(async () => {})

    await createAnalyticsComponent({ config, logger, fs })

    expect(appendSpy).not.toHaveBeenCalled()
  })

  test('should not create identify user if DCL_DISABLE_ANALYTICS is set to true', async () => {
    const fs = createFsComponent()
    const config = createRecordConfigComponent({
      DCL_DISABLE_ANALYTICS: 'true'
    })
    const logger = createStderrCliLogger()
    const appendSpy = jest.spyOn(fs, 'appendFile').mockImplementation(async () => {})

    const analytics = await createAnalyticsComponent({ config, logger, fs })
    analytics.track('Build scene', {} as any)
    await analytics.stop()
    expect(appendSpy).not.toHaveBeenCalled()
  })

  it('should track event', async () => {
    const fs = createFsComponent()
    const config = createRecordConfigComponent({
      DCL_ANON_ID: 'fb3f84b2-4ddc-4a7e-96bf-1e8992c294dd'
    })
    const logger = createStderrCliLogger()
    const analytics = await createAnalyticsComponent({ config, logger, fs })
    if (!('get' in analytics)) throw new Error('analytics.get is not defined')

    // whatever @dcl/sdk resolves to from here: nothing under npm, the workspace package through the
    // NODE_PATH that pnpm's bin shims set
    const sdkVersion = await getInstalledPackageVersion({ fs }, '@dcl/sdk', process.cwd())

    const trackSpy = jest.spyOn(analytics.get(), 'track')
    analytics.track('Scene created', { projectType: '', url: '' })
    expect(trackSpy).toHaveBeenCalledWith({
      userId: 'sdk-commands-user',
      event: 'Scene created',
      properties: {
        projectType: '',
        url: '',
        os: process.platform,
        nodeVersion: process.version,
        cliVersion: '7.0.0',
        isCI: isCI(),
        isEditor: false,
        devId: 'fb3f84b2-4ddc-4a7e-96bf-1e8992c294dd',
        appId: null,
        projectId: null,
        ecs: {
          ecsVersion: 'ecs7',
          packageVersion: sdkVersion
        }
      }
    })
  })

  it('should wait for promises to finished', async () => {
    const fs = createFsComponent()
    const config = createRecordConfigComponent({
      DCL_ANON_ID: 'fb3f84b2-4ddc-4a7e-96bf-1e8992c294dd'
    })
    expect(await requireStringConfig({ config }, 'DCL_ANON_ID')).toEqual('fb3f84b2-4ddc-4a7e-96bf-1e8992c294dd')
    const logger = createStderrCliLogger()
    const analytics = await createAnalyticsComponent({ config, logger, fs })
    if (!('get' in analytics)) throw new Error('analytics.get is not defined')

    const spyTrack = jest.spyOn(analytics.get(), 'track')
    analytics.track('Build scene', {} as any)
    await analytics.stop()
    expect(spyTrack).toHaveBeenCalled()
  })
})
