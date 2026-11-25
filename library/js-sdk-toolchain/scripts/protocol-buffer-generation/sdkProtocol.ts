import { copySync, readFileSync, removeSync, writeFileSync } from 'fs-extra'
import path from 'path'
import { getFilePathsSync } from '../utils/getFilePathsSync'

const BEGIN = '// sdk:hidden-begin'
const END = '// sdk:hidden-end'

/**
 * The protocol the SDK is generated from: the `.proto` text without what sits between a
 * `// sdk:hidden-begin` line and a `// sdk:hidden-end` line. The Explorer builds from the whole file.
 */
export function withoutHiddenBlocks(proto: string, fileName: string): string {
  const kept: string[] = []
  let hiddenSince: number | undefined
  proto.split('\n').forEach((line, index) => {
    const marker = line.trim()
    if (marker === BEGIN) {
      if (hiddenSince !== undefined) throw new Error(`${fileName}:${index + 1}: ${BEGIN} inside a hidden block`)
      hiddenSince = index
    } else if (marker === END) {
      if (hiddenSince === undefined) throw new Error(`${fileName}:${index + 1}: ${END} without a ${BEGIN}`)
      hiddenSince = undefined
    } else if (hiddenSince === undefined) {
      kept.push(line)
    }
  })
  if (hiddenSince !== undefined) throw new Error(`${fileName}:${hiddenSince + 1}: ${BEGIN} is never closed`)
  return kept.join('\n')
}

export function writeSdkProtocol(protocolPath: string, sdkProtocolPath: string) {
  removeSync(sdkProtocolPath)
  copySync(protocolPath, sdkProtocolPath, { dereference: true })
  for (const file of getFilePathsSync(sdkProtocolPath, true)) {
    if (!file.endsWith('.proto')) continue
    const filePath = path.resolve(sdkProtocolPath, file)
    writeFileSync(filePath, withoutHiddenBlocks(readFileSync(filePath).toString(), file))
  }
}
