// A static file handler for `node:http`, for the inspector's `public` folder. It imports only Node
// built-ins and uses only erasable TypeScript, so `scripts/serve-static.js` can import this file
// directly.
//
// It reads files with `fs.promises.stat`, `fs.promises.realpath` and `fs.createReadStream`, looked
// up on the `fs` object at call time: in a packaged app the folder is inside `app.asar`, and
// Electron serves an archive only through the `fs` functions it replaces.
import fs from 'node:fs';
import path from 'node:path';
import type { IncomingMessage, ServerResponse } from 'node:http';

export type StaticHandlerOptions = {
  root: string;
  /** Sent with every response, errors included. */
  headers?: Record<string, string>;
};

type Target = {
  file: string;
  segments: string[];
  search: string;
  trailingSlash: boolean;
};

const UTF8 = '; charset=utf-8';

// `application/wasm` is what `WebAssembly.instantiateStreaming` requires.
const CONTENT_TYPES: Record<string, string> = {
  html: 'text/html' + UTF8,
  htm: 'text/html' + UTF8,
  js: 'application/javascript' + UTF8,
  mjs: 'application/javascript' + UTF8,
  css: 'text/css' + UTF8,
  json: 'application/json' + UTF8,
  map: 'application/json' + UTF8,
  txt: 'text/plain' + UTF8,
  md: 'text/markdown' + UTF8,
  xml: 'application/xml',
  webmanifest: 'application/manifest+json',
  wasm: 'application/wasm',
  glb: 'model/gltf-binary',
  gltf: 'model/gltf+json',
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  svg: 'image/svg+xml',
  ico: 'image/x-icon',
  woff: 'font/woff',
  woff2: 'font/woff2',
  ttf: 'font/ttf',
  otf: 'font/otf',
  mp3: 'audio/mpeg',
  ogg: 'audio/ogg',
  wav: 'audio/wav',
  mp4: 'video/mp4',
  webm: 'video/webm',
};

const NOT_FOUND_CODES = new Set(['ENOENT', 'ENOTDIR', 'ENAMETOOLONG', 'ELOOP', 'EACCES', 'EPERM']);

export function contentType(file: string): string {
  const extension = path.extname(file).slice(1).toLowerCase();
  return CONTENT_TYPES[extension] ?? 'application/octet-stream';
}

/** True when `file` is `root` or a path below it. `<root>-other` is not below `<root>`. */
export function isInside(root: string, file: string): boolean {
  const relative = path.relative(root, file);
  return relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative);
}

/**
 * Maps a request target to a path below `root`. Returns the status to answer with when the target
 * is malformed (400) or can name nothing that is served (404).
 *
 * The path is decoded once. A segment that starts with a dot is refused, which covers `.`, `..`,
 * their encoded forms and dot files. A browser resolves dot segments before it sends a request,
 * so no page needs them.
 */
export function resolveTarget(root: string, url: string): Target | 400 | 404 {
  const [, rawPath, search = ''] = /^([^?#]*)(\?[^#]*)?/.exec(url) ?? [];
  if (!rawPath?.startsWith('/')) return 400;

  let decoded: string;
  try {
    decoded = decodeURIComponent(rawPath);
  } catch {
    return 400;
  }
  // A backslash is a separator for `path.resolve` on Windows.
  if (decoded.includes('\0') || decoded.includes('\\')) return 400;

  const segments = decoded.split('/').filter(segment => segment !== '');
  if (segments.some(segment => segment.startsWith('.'))) return 404;

  // On Windows a segment such as `D:` makes `path.resolve` leave the root.
  const file = path.resolve(root, ...segments);
  if (!isInside(root, file)) return 404;

  return { file, segments, search, trailingSlash: decoded.endsWith('/') };
}

function reply(res: ServerResponse, status: number, headers: Record<string, string> = {}) {
  res.writeHead(status, { ...headers, 'Content-Length': '0' });
  res.end();
}

function isFresh(req: IncomingMessage, etag: string, mtimeMs: number): boolean {
  const noneMatch = req.headers['if-none-match'];
  if (noneMatch) {
    const weak = (tag: string) => tag.trim().replace(/^W\//, '');
    return noneMatch.trim() === '*' || noneMatch.split(',').some(tag => weak(tag) === weak(etag));
  }
  const since = Date.parse(req.headers['if-modified-since'] ?? '');
  return !Number.isNaN(since) && Math.floor(mtimeMs / 1000) * 1000 <= since;
}

async function serve(root: string, req: IncomingMessage, res: ServerResponse) {
  if (req.method !== 'GET' && req.method !== 'HEAD') {
    return reply(res, 405, { Allow: 'GET, HEAD' });
  }

  const target = resolveTarget(root, req.url ?? '');
  if (typeof target === 'number') return reply(res, target);

  // A symbolic link may point anywhere, so the check is repeated on the real paths.
  const realRoot = await fs.promises.realpath(root);
  const realFile = async (candidate: string) => {
    const real = await fs.promises.realpath(candidate);
    return isInside(realRoot, real) ? real : null;
  };

  let file = await realFile(target.file);
  if (!file) return reply(res, 404);
  let stats = await fs.promises.stat(file);

  if (stats.isDirectory()) {
    if (!target.trailingSlash && target.segments.length > 0) {
      // The location is built from the segments, never from the raw target: `//host/` would
      // send the browser to another host.
      const location = `/${target.segments.map(encodeURIComponent).join('/')}/${target.search}`;
      return reply(res, 302, { Location: location });
    }
    file = await realFile(path.join(file, 'index.html'));
    if (!file) return reply(res, 404);
    stats = await fs.promises.stat(file);
  } else if (target.trailingSlash) {
    return reply(res, 404);
  }
  if (!stats.isFile()) return reply(res, 404);

  const etag = `W/"${stats.size.toString(16)}-${Math.floor(stats.mtimeMs).toString(16)}"`;
  const cacheHeaders = {
    'Cache-Control': 'max-age=3600',
    'Last-Modified': stats.mtime.toUTCString(),
    ETag: etag,
  };
  if (isFresh(req, etag, stats.mtimeMs)) {
    res.writeHead(304, cacheHeaders);
    res.end();
    return;
  }

  const headers = {
    ...cacheHeaders,
    'Content-Type': contentType(file),
    'Content-Length': String(stats.size),
  };
  if (req.method === 'HEAD' || stats.size === 0) {
    res.writeHead(200, headers);
    res.end();
    return;
  }

  // `end` keeps the body at the announced length when the file grows after the stat.
  const stream = fs.createReadStream(file, { end: stats.size - 1 });
  stream.once('open', () => {
    res.writeHead(200, headers);
    stream.pipe(res);
  });
  stream.once('error', () => {
    if (res.headersSent) res.destroy();
    else reply(res, 500);
  });
  res.once('close', () => stream.destroy());
}

/** Answers GET and HEAD with the files below `root`. It has no directory listing. */
export function createStaticHandler({ root, headers = {} }: StaticHandlerOptions) {
  const absoluteRoot = path.resolve(root);

  return async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    for (const [name, value] of Object.entries(headers)) res.setHeader(name, value);
    try {
      await serve(absoluteRoot, req, res);
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code ?? '';
      if (res.headersSent) res.destroy();
      else reply(res, NOT_FOUND_CODES.has(code) ? 404 : 500);
    }
  };
}
