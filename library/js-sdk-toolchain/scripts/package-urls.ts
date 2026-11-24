/**
 * @param baseUrl - the folder that holds this build's tarballs
 * @param version - the version every workspace package is packed at
 * @returns the dependencies, each `workspace:` range replaced by its `pnpm pack` tarball URL
 */
export function pinWorkspaceDependencies(
  dependencies: Record<string, string>,
  baseUrl: string,
  version: string
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(dependencies).map(([name, range]) => [
      name,
      range.startsWith('workspace:') ? `${baseUrl}/${name.replace('@', '').replace('/', '-')}-${version}.tgz` : range
    ])
  )
}
