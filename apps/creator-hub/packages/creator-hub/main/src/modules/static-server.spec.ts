import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import type { AddressInfo } from 'node:net';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { contentType, createStaticHandler, isInside, resolveTarget } from './static-server';

const HEADERS = { 'Cross-Origin-Opener-Policy': 'same-origin', 'X-Test': 'yes' };

type Answer = { status: number; headers: http.IncomingHttpHeaders; body: string };

let folder: string;
let root: string;
let server: http.Server;
let port: number;

// `http.request` sends the path as given: it does not resolve `..` or decode anything.
function request(
  target: string,
  options: { method?: string; headers?: Record<string, string> } = {},
): Promise<Answer> {
  return new Promise((resolve, reject) => {
    const req = http.request(
      { host: '127.0.0.1', port, path: target, method: options.method, headers: options.headers },
      res => {
        const chunks: Buffer[] = [];
        res.on('data', chunk => chunks.push(chunk));
        res.on('end', () =>
          resolve({
            status: res.statusCode ?? 0,
            headers: res.headers,
            body: Buffer.concat(chunks).toString('latin1'),
          }),
        );
      },
    );
    req.on('error', reject);
    req.end();
  });
}

beforeAll(async () => {
  folder = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'static-server-')));
  root = path.join(folder, 'root');
  const write = (name: string, contents: string) => {
    fs.mkdirSync(path.dirname(path.join(folder, name)), { recursive: true });
    fs.writeFileSync(path.join(folder, name), contents);
  };
  write('root/index.html', '<title>root</title>');
  write('root/engine/index.html', '<title>engine</title>');
  write('root/engine/pkg/engine_bg.wasm', '\0asm');
  write('root/engine/pkg/engine.js', 'export {};');
  write('root/assets/model.glb', 'glTF');
  write('root/assets/bundle.js.map', '{}');
  write('root/assets/data.json', '{"a":1}');
  write('root/assets/with space.txt', 'space');
  write('root/assets/empty.txt', '');
  write('root/realm/about', '{}');
  write('root/no-index/file.txt', 'file');
  write('root/.env', 'SECRET');
  write('root/.git/config', 'SECRET');
  write('root-other/secret.txt', 'SECRET');
  write('outside.txt', 'SECRET');
  fs.symlinkSync(path.join(folder, 'outside.txt'), path.join(root, 'link-out.txt'));
  fs.symlinkSync(path.join(folder, 'root-other'), path.join(root, 'link-dir'));
  fs.symlinkSync(path.join(root, 'assets', 'data.json'), path.join(root, 'link-in.json'));
  fs.mkdirSync(path.join(root, 'index-out'));
  fs.symlinkSync(path.join(folder, 'outside.txt'), path.join(root, 'index-out', 'index.html'));

  server = http.createServer(createStaticHandler({ root, headers: HEADERS }));
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  port = (server.address() as AddressInfo).port;
});

afterAll(async () => {
  await new Promise(resolve => server.close(resolve));
  fs.rmSync(folder, { recursive: true, force: true });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('isInside', () => {
  it('should accept the root and the paths below it', () => {
    expect(isInside(path.resolve('/srv/root'), path.resolve('/srv/root'))).toBe(true);
    expect(isInside(path.resolve('/srv/root'), path.resolve('/srv/root/a/b.js'))).toBe(true);
    expect(isInside(path.resolve('/srv/root'), path.resolve('/srv/root/..data'))).toBe(true);
  });

  it('should refuse the parent, a sibling with the root as its prefix and another tree', () => {
    expect(isInside(path.resolve('/srv/root'), path.resolve('/srv'))).toBe(false);
    expect(isInside(path.resolve('/srv/root'), path.resolve('/srv/root-other/x'))).toBe(false);
    expect(isInside(path.resolve('/srv/root'), path.resolve('/etc/passwd'))).toBe(false);
  });
});

describe('resolveTarget', () => {
  const base = path.resolve('/srv/root');

  describe('when the target names a path below the root', () => {
    it('should resolve it and keep the query apart', () => {
      expect(resolveTarget(base, '/a/b%20c.js?x=1#frag')).toEqual({
        file: path.join(base, 'a', 'b c.js'),
        segments: ['a', 'b c.js'],
        search: '?x=1',
        trailingSlash: false,
      });
    });

    it('should ignore empty segments and report a trailing slash', () => {
      expect(resolveTarget(base, '//a//b/')).toEqual({
        file: path.join(base, 'a', 'b'),
        segments: ['a', 'b'],
        search: '',
        trailingSlash: true,
      });
    });

    it('should decode the path only once', () => {
      expect(resolveTarget(base, '/%252e%252e/x')).toMatchObject({
        file: path.join(base, '%2e%2e', 'x'),
      });
    });
  });

  describe('when the target is malformed', () => {
    it.each([
      ['an empty target', ''],
      ['a target with no leading slash', 'index.html'],
      ['an absolute URL', 'http://example.test/index.html'],
      ['an invalid escape', '/%zz'],
      ['an invalid UTF-8 sequence', '/%c0%ae%c0%ae/x'],
      ['an encoded NUL', '/index.html%00.png'],
      ['a backslash', '/..\\outside.txt'],
      ['an encoded backslash', '/..%5coutside.txt'],
    ])('should answer 400 for %s', (_name, target) => {
      expect(resolveTarget(base, target)).toBe(400);
    });
  });

  describe('when a segment starts with a dot', () => {
    it.each([
      ['a parent segment', '/../outside.txt'],
      ['a parent segment after a folder', '/a/../../outside.txt'],
      ['an encoded parent segment', '/%2e%2e/outside.txt'],
      ['a half-encoded parent segment', '/.%2e/outside.txt'],
      ['an encoded slash after a parent segment', '/..%2foutside.txt'],
      ['a parent segment that stays inside', '/a/../b.js'],
      ['a current-folder segment', '/a/./b.js'],
      ['a dot file', '/.env'],
      ['a dot folder', '/.git/config'],
      ['three dots', '/.../x'],
    ])('should answer 404 for %s', (_name, target) => {
      expect(resolveTarget(base, target)).toBe(404);
    });
  });
});

describe('contentType', () => {
  it.each([
    ['engine_bg.wasm', 'application/wasm'],
    ['bundle.js', 'application/javascript; charset=utf-8'],
    ['worker.MJS', 'application/javascript; charset=utf-8'],
    ['index.html', 'text/html; charset=utf-8'],
    ['bundle.css', 'text/css; charset=utf-8'],
    ['data.json', 'application/json; charset=utf-8'],
    ['bundle.js.map', 'application/json; charset=utf-8'],
    ['model.glb', 'model/gltf-binary'],
    ['icon.svg', 'image/svg+xml'],
    ['font.woff2', 'font/woff2'],
    ['about', 'application/octet-stream'],
    ['file.unknown', 'application/octet-stream'],
  ])('should map %s to %s', (file, type) => {
    expect(contentType(file)).toBe(type);
  });
});

describe('createStaticHandler', () => {
  describe('when the request is for a file', () => {
    it('should send the file with its type, its length, the cache headers and the custom headers', async () => {
      const answer = await request('/engine/pkg/engine_bg.wasm');

      expect(answer.status).toBe(200);
      expect(answer.body).toBe('\0asm');
      expect(answer.headers).toMatchObject({
        'content-type': 'application/wasm',
        'content-length': '4',
        'cache-control': 'max-age=3600',
        'cross-origin-opener-policy': 'same-origin',
        'x-test': 'yes',
      });
      expect(answer.headers.etag).toMatch(/^W\/"[0-9a-f]+-[0-9a-f]+"$/);
      expect(answer.headers['last-modified']).toBeDefined();
    });

    it('should ignore the query string', async () => {
      const answer = await request('/assets/data.json?v=2');

      expect(answer.status).toBe(200);
      expect(answer.body).toBe('{"a":1}');
    });

    it('should decode an escaped name', async () => {
      expect((await request('/assets/with%20space.txt')).body).toBe('space');
    });

    it('should send a file with no extension as application/octet-stream', async () => {
      const answer = await request('/realm/about');

      expect(answer.status).toBe(200);
      expect(answer.headers['content-type']).toBe('application/octet-stream');
    });

    it('should send an empty file', async () => {
      const answer = await request('/assets/empty.txt');

      expect(answer.status).toBe(200);
      expect(answer.headers['content-length']).toBe('0');
      expect(answer.body).toBe('');
    });

    it('should send the whole file when the request has a Range header', async () => {
      const answer = await request('/assets/data.json', { headers: { Range: 'bytes=0-1' } });

      expect(answer.status).toBe(200);
      expect(answer.body).toBe('{"a":1}');
      expect(answer.headers['accept-ranges']).toBeUndefined();
    });

    it('should answer 404 when the path has a trailing slash', async () => {
      expect((await request('/index.html/')).status).toBe(404);
    });
  });

  describe('when the request method is HEAD', () => {
    it('should send the headers of the file and no body', async () => {
      const answer = await request('/engine/pkg/engine.js', { method: 'HEAD' });

      expect(answer.status).toBe(200);
      expect(answer.body).toBe('');
      expect(answer.headers).toMatchObject({
        'content-type': 'application/javascript; charset=utf-8',
        'content-length': '10',
      });
    });
  });

  describe('when the request method is not GET or HEAD', () => {
    it.each(['POST', 'PUT', 'DELETE', 'OPTIONS', 'PATCH'])(
      'should answer 405 for %s',
      async method => {
        const answer = await request('/index.html', { method });

        expect(answer.status).toBe(405);
        expect(answer.headers.allow).toBe('GET, HEAD');
        expect(answer.headers['x-test']).toBe('yes');
        expect(answer.body).toBe('');
      },
    );
  });

  describe('when the request is for a folder', () => {
    it('should send the index.html of the root', async () => {
      expect((await request('/')).body).toBe('<title>root</title>');
    });

    it('should send the index.html of a folder', async () => {
      const answer = await request('/engine/');

      expect(answer.status).toBe(200);
      expect(answer.headers['content-type']).toBe('text/html; charset=utf-8');
      expect(answer.body).toBe('<title>engine</title>');
    });

    it('should redirect to the path with a trailing slash and keep the query', async () => {
      const answer = await request('/engine?x=1');

      expect(answer.status).toBe(302);
      expect(answer.headers.location).toBe('/engine/?x=1');
    });

    it('should not build a redirect that names another host', async () => {
      const answer = await request('//engine');

      expect(answer.status).toBe(302);
      expect(answer.headers.location).toBe('/engine/');
    });

    it('should answer 404 and list nothing when the folder has no index.html', async () => {
      const answer = await request('/no-index/');

      expect(answer.status).toBe(404);
      expect(answer.body).toBe('');
    });
  });

  describe('when the request has a validator', () => {
    it('should answer 304 when the ETag matches', async () => {
      const first = await request('/assets/data.json');
      const second = await request('/assets/data.json', {
        headers: { 'If-None-Match': String(first.headers.etag) },
      });

      expect(second.status).toBe(304);
      expect(second.body).toBe('');
      expect(second.headers['cache-control']).toBe('max-age=3600');
    });

    it('should send the file when the ETag does not match', async () => {
      const answer = await request('/assets/data.json', {
        headers: { 'If-None-Match': 'W/"0-0"' },
      });

      expect(answer.status).toBe(200);
    });

    it('should answer 304 when the file is not newer than If-Modified-Since', async () => {
      const first = await request('/assets/data.json');
      const second = await request('/assets/data.json', {
        headers: { 'If-Modified-Since': String(first.headers['last-modified']) },
      });

      expect(second.status).toBe(304);
    });

    it('should send the file when it is newer than If-Modified-Since', async () => {
      const answer = await request('/assets/data.json', {
        headers: { 'If-Modified-Since': 'Thu, 01 Jan 1970 00:00:00 GMT' },
      });

      expect(answer.status).toBe(200);
    });
  });

  describe('when the request names nothing that is served', () => {
    it.each([
      ['a missing file', '/missing.js', 404],
      ['a path below a file', '/index.html/x', 404],
      ['a parent segment', '/../outside.txt', 404],
      [
        'a parent segment to a sibling with the root as its prefix',
        '/../root-other/secret.txt',
        404,
      ],
      ['an encoded parent segment', '/%2e%2e/outside.txt', 404],
      ['an encoded slash after a parent segment', '/..%2foutside.txt', 404],
      ['a twice-encoded parent segment', '/%252e%252e/outside.txt', 404],
      ['a dot file', '/.env', 404],
      ['a dot folder', '/.git/config', 404],
      ['a backslash', '/..\\outside.txt', 400],
      ['an encoded backslash', '/..%5coutside.txt', 400],
      ['an encoded NUL', '/index.html%00', 400],
      ['an invalid escape', '/%zz', 400],
      ['a symbolic link to a file outside the root', '/link-out.txt', 404],
      ['a file behind a symbolic link to a folder outside the root', '/link-dir/secret.txt', 404],
      ['a folder whose index.html is a symbolic link to the outside', '/index-out/', 404],
    ])('should answer with an empty body for %s', async (_name, target, status) => {
      const answer = await request(target);

      expect(answer.status).toBe(status);
      expect(answer.body).toBe('');
      expect(answer.headers['x-test']).toBe('yes');
      expect(answer.headers['cache-control']).toBeUndefined();
    });

    it('should follow a symbolic link that stays inside the root', async () => {
      const answer = await request('/link-in.json');

      expect(answer.status).toBe(200);
      expect(answer.body).toBe('{"a":1}');
    });
  });

  describe('when the root folder does not exist', () => {
    it('should answer 404', async () => {
      const handler = createStaticHandler({ root: path.join(folder, 'absent') });
      const other = http.createServer(handler);
      await new Promise<void>(resolve => other.listen(0, '127.0.0.1', resolve));
      const { port: otherPort } = other.address() as AddressInfo;

      const status = await new Promise<number>((resolve, reject) => {
        http
          .get({ host: '127.0.0.1', port: otherPort, path: '/' }, res => {
            res.resume();
            resolve(res.statusCode ?? 0);
          })
          .on('error', reject);
      });
      await new Promise(resolve => other.close(resolve));

      expect(status).toBe(404);
    });
  });

  describe('when the file cannot be read', () => {
    it('should answer 500 with an empty body and no cache headers', async () => {
      const createReadStream = fs.createReadStream;
      vi.spyOn(fs, 'createReadStream').mockImplementation(() =>
        createReadStream(path.join(folder, 'absent-file')),
      );

      const answer = await request('/assets/data.json');

      expect(answer.status).toBe(500);
      expect(answer.body).toBe('');
      expect(answer.headers['cache-control']).toBeUndefined();
    });

    it('should answer 500 when the stat fails with an unexpected error', async () => {
      vi.spyOn(fs.promises, 'stat').mockRejectedValue(
        Object.assign(new Error('boom'), { code: 'EIO' }),
      );

      const answer = await request('/assets/data.json');

      expect(answer.status).toBe(500);
      expect(answer.body).toBe('');
    });
  });
});
