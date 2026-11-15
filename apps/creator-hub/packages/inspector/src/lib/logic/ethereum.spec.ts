import { describe, expect, it } from 'vitest';

import { checkAddressCheckSum, formatAddress, isAddress } from './ethereum';

describe('when formatting an address for display', () => {
  it('should keep the first twelve and last eight characters', () => {
    expect(formatAddress('0x0000000000000000000000000000000000000001')).toBe(
      '0x0000000000...00000001',
    );
  });

  it('should return an empty value untouched', () => {
    expect(formatAddress('')).toBe('');
  });

  it('should return a value that is not an address verbatim', () => {
    expect(formatAddress('not-an-address')).toBe('not-an-address');
  });

  it('should preserve the checksum casing of a mixed-case address', () => {
    expect(formatAddress('0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed')).toBe(
      '0x5aAeb6053F...Ef1BeAed',
    );
  });

  it('should format a prefixless address, which isAddress accepts', () => {
    expect(formatAddress('5aaeb6053f3e94c9b9a09f33669435e7ef1beaed')).toBe(
      '5aaeb6053f3e...ef1beaed',
    );
  });
});

describe('when validating an address', () => {
  it.each([
    ['lowercase', '0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed'],
    ['upper-case hex after a 0x prefix', '0x5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED'],
    ['fully upper-case, prefix included', '0X5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED'],
    ['EIP-55 checksummed', '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed'],
    ['prefixless', '5aaeb6053f3e94c9b9a09f33669435e7ef1beaed'],
  ])('should accept a %s address', (_, address) => {
    expect(isAddress(address)).toBe(true);
  });

  it('should reject a mixed-case address whose checksum is wrong', () => {
    expect(isAddress('0x5aAeb6053f3e94c9b9a09f33669435e7ef1beaed')).toBe(false);
  });

  it('should reject a shortened display address', () => {
    expect(isAddress('0x5aAeb6053F...Ef1BeAed')).toBe(false);
  });
});

describe('when checking the checksum of an address', () => {
  it.each([
    '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed',
    '0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359',
    '0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB',
    '0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb',
  ])('should accept the EIP-55 reference address %s', address => {
    expect(checkAddressCheckSum(address)).toBe(true);
  });

  it('should reject a reference address with the case of one letter changed', () => {
    expect(checkAddressCheckSum('0xFB6916095ca1df60bB79Ce92cE3Ea74c37c5d359')).toBe(false);
  });
});
