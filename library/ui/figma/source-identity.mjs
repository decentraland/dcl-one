import { createHash } from 'node:crypto'

/** The cache marker and generation records share this exact path-sorted digest. */
export function digestTableHash(digests) {
  return createHash('sha256')
    .update(
      Object.keys(digests)
        .sort()
        .map((file) => `${file}\0${digests[file]}\n`)
        .join('')
    )
    .digest('hex')
}

export function componentSourceIdentity(source) {
  if (
    source?.repository !== 'https://github.com/decentraland/figma-design-system' ||
    typeof source.revision !== 'string' ||
    !/^[a-f0-9]{40}$/.test(source.revision) ||
    typeof source.snapshot?.revision !== 'string' ||
    !/^[a-f0-9]{40}$/.test(source.snapshot.revision) ||
    !source.snapshot?.digests ||
    typeof source.snapshot.digests !== 'object' ||
    Array.isArray(source.snapshot.digests) ||
    !Object.keys(source.snapshot.digests).length ||
    Object.values(source.snapshot.digests).some(
      (value) => typeof value !== 'string' || !/^[a-f0-9]{64}$/.test(value)
    )
  )
    throw new Error('Invalid verified component source identity')
  return {
    repository: source.repository,
    codeRevision: source.revision,
    snapshotRevision: source.snapshot.revision,
    digestTableHash: digestTableHash(source.snapshot.digests)
  }
}
