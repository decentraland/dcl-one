import { extname } from 'path';

const DEFAULT_CONTENT_TYPE = 'application/octet-stream';

/**
 * The content type of each kind of file that the packs hold. The stored files are immutable, so
 * the values stay as they were first uploaded, `.ts` included.
 */
export const CONTENT_TYPES: Record<string, string> = {
  '.glb': 'model/gltf-binary',
  '.gltf': 'model/gltf+json',
  '.jpeg': 'image/jpeg',
  '.jpg': 'image/jpeg',
  '.json': 'application/json',
  '.md': 'text/markdown',
  '.mp3': 'audio/mpeg',
  '.mp4': 'video/mp4',
  '.ogg': 'audio/ogg',
  '.png': 'image/png',
  '.ts': 'video/mp2t',
  '.tsx': DEFAULT_CONTENT_TYPE,
  '.wav': 'audio/wave',
  '.webp': 'image/webp',
};

export function contentType(file: string): string {
  return CONTENT_TYPES[extname(file).toLowerCase()] ?? DEFAULT_CONTENT_TYPE;
}
