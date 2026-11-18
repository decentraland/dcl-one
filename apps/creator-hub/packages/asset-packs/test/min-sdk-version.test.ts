import { describe, expect, it } from 'vitest';
import { assertValidMinSdkVersion } from '../src/types';

describe('assertValidMinSdkVersion', () => {
  it('should accept a full version', () => {
    expect(() => assertValidMinSdkVersion('Skybox', '7.32.0')).not.toThrow();
  });

  it('should accept an asset without one', () => {
    expect(() => assertValidMinSdkVersion('Door', undefined)).not.toThrow();
  });

  it('should reject a version the editor cannot compare, so it never silently lets the item through', () => {
    expect(() => assertValidMinSdkVersion('Skybox', '7.32')).toThrow('Invalid minSdkVersion');
    expect(() => assertValidMinSdkVersion('Skybox', 7.32)).toThrow('Invalid minSdkVersion');
  });
});
