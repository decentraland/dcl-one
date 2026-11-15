import { describe, expect, it } from 'vitest';

import { signedFetch } from './signedFetch';

describe('signedFetch', () => {
  it('should be a function made from the named factory of the package', () => {
    expect(typeof signedFetch).toBe('function');
  });
});
