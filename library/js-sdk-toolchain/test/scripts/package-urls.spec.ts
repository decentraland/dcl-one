import { pinWorkspaceDependencies } from '../../scripts/package-urls'

describe('when a pull request build pins the dependencies of a packed package', () => {
  let dependencies: Record<string, string>
  let pinned: Record<string, string>

  beforeEach(() => {
    dependencies = {
      '@dcl/ecs': 'workspace:*',
      '@dcl/sdk-commands': 'workspace:*',
      '@dcl/ecs-math': '2.1.0'
    }
    pinned = pinWorkspaceDependencies(dependencies, 'https://bucket.example/branch/feat/x', '7.33.1-5.commit-abc1234')
  })

  it('should point each workspace package at the tarball pnpm pack names in the build folder', () => {
    expect(pinned['@dcl/sdk-commands']).toBe(
      'https://bucket.example/branch/feat/x/dcl-sdk-commands-7.33.1-5.commit-abc1234.tgz'
    )
  })

  it('should keep every registry dependency as it is', () => {
    expect(pinned['@dcl/ecs-math']).toBe('2.1.0')
  })
})
