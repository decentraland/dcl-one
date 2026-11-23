import { describe, expect, it } from 'vitest';

import { describeError } from './describe';

describe('describeError', () => {
  it('should replace POSIX and Windows paths', () => {
    expect(
      describeError({
        message:
          "EACCES: permission denied, open '/home/jane/scene/main.ts' and C:\\Users\\jane\\x.glb",
      }),
    ).toBe("EACCES: permission denied, open '<path>' and <path>");
  });

  it('should replace a Windows home folder whose name has a space', () => {
    expect(
      describeError({
        message: 'Cannot read C:\\Users\\Jane Doe\\Documents\\My Scenes\\park\\scene.json now',
      }),
    ).toBe('Cannot read <path> now');
  });

  it('should replace a OneDrive folder, quoted or bare', () => {
    expect(
      describeError({
        message:
          "EPERM: operation not permitted, mkdir 'C:\\Users\\jane\\OneDrive - Acme Corp\\Scenes'",
      }),
    ).toBe("EPERM: operation not permitted, mkdir '<path>'");
    expect(
      describeError({ message: 'at C:\\Users\\jane\\OneDrive - Acme Corp\\Scenes\\x.glb' }),
    ).toBe('at <path>');
  });

  it('should keep a URL host and drop its path and query token', () => {
    expect(
      describeError({
        message: 'Request to https://peer.example.org/content/entities?token=abc123secret failed',
      }),
    ).toBe('Request to https://peer.example.org/<path> failed');
    expect(describeError({ message: 'GET https://peer.example.org failed' })).toBe(
      'GET https://peer.example.org failed',
    );
  });

  it('should replace an email address', () => {
    expect(describeError({ message: 'No wallet for jane.doe@example.com' })).toBe(
      'No wallet for <email>',
    );
  });

  it('should keep a message with no path as it is', () => {
    expect(describeError({ message: 'Request failed: 503' })).toBe('Request failed: 503');
  });

  it('should cap the length', () => {
    expect(describeError({ message: 'x'.repeat(500) })).toHaveLength(200);
  });

  it('should fall back to the name, then to unknown', () => {
    expect(describeError({ name: 'AbortError' })).toBe('AbortError');
    expect(describeError(undefined)).toBe('unknown');
  });
});
