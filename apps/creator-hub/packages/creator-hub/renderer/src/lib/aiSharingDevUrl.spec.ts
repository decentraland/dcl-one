import { describe, expect, it } from 'vitest';

import { devAiSharingUrl } from './aiSharingDevUrl';

describe('devAiSharingUrl', () => {
  describe('when a dev build has VITE_AI_SHARING_API_URL', () => {
    it('should return it, trimmed', () => {
      expect(
        devAiSharingUrl({ DEV: true, VITE_AI_SHARING_API_URL: ' http://localhost:3000 ' }),
      ).toBe('http://localhost:3000');
    });
  });

  describe('when a dev build has no override', () => {
    it('should return nothing', () => {
      expect(devAiSharingUrl({ DEV: true })).toBe('');
      expect(devAiSharingUrl({ DEV: true, VITE_AI_SHARING_API_URL: '  ' })).toBe('');
    });
  });

  describe('when the build is packaged or a release', () => {
    it('should ignore the variable even if it was set at build time', () => {
      expect(
        devAiSharingUrl({ DEV: false, VITE_AI_SHARING_API_URL: 'http://localhost:3000' }),
      ).toBe('');
    });
  });
});
