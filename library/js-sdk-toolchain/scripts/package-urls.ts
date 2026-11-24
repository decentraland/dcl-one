/**
 * @param baseUrl - the folder that holds one folder per package of this build
 * @param version - the version every workspace package is packed at
 * @returns the dependencies, each `workspace:` range replaced by the URL of its `pnpm pack` tarball in its package's folder
 */
export function pinWorkspaceDependencies(
  dependencies: Record<string, string>,
  baseUrl: string,
  version: string
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(dependencies).map(([name, range]) => [
      name,
      range.startsWith('workspace:')
        ? `${baseUrl}/${name}/${name.replace('@', '').replace('/', '-')}-${version}.tgz`
        : range
    ])
  )
}
