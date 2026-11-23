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
