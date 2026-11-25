// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { generateTree, hashPair, leafOf } from './contentHashTree'

// Roots and proofs that @dcl/content-hash-tree 1.1.4 gives for these inputs: the third party registry and the
// Catalyst check against that tree, so this one must not differ.
const hashes = (...numbers: number[]) => numbers.map(n => `bafkrei${String(n).padStart(4, '0')}`)

const REFERENCE = {
  one: { input: hashes(1), root: '0x78163848a4d1f01159b7f72171adc0b9385fb847d30981b1938541573f937487' },
  two: { input: hashes(2, 1), root: '0xdfe4c39071ee02142d8706d14247b57d6ec9988c7206c99650be9e541dde570a' },
  three: { input: hashes(3, 1, 2), root: '0xd622fd0e2b8fc41f7ff6eff9e417d7380984f1f2bec8fff0520e03c3b38b41e1' },
  five: { input: hashes(5, 3, 1, 4, 2), root: '0x84a0980fe45462b7b42d2ab6b0a3b48074035932e9abb094109ea68bdb7e3bb9' },
  eight: {
    input: hashes(8, 7, 6, 5, 4, 3, 2, 1),
    root: '0x04ca953dc53c7a9500c579420ec532631b3416612ad8db5fce926bcecd8b19cd'
  },
  repeated: { input: hashes(1, 2, 1), root: '0xb9e178294d67e37ef6c089327e35b5d58955d43a84925d5bcca83dc129f1cffb' },
  mixedCase: {
    input: ['QmB', 'bafA', 'Qma', 'bafz', '0x1F'],
    root: '0x7b9c1f8cb53d9587d15489a9dd2f173ffc04cc547a36f1ef5a4f3f9de0406b4d'
  }
}

const REFERENCE_PROOFS = {
  five: {
    bafkrei0001: {
      index: 0,
      proof: [
        '0x9bca6d45e905c6fcd9ad97012ce3933d65235dbf6511e888942cf000cda1672c',
        '0xc945d957e1d5e8ee0e3fe0b61fc6f5d567620853461020a1ecae426570945e6c',
        '0xcb431c434637e058084f23fc3798499aa631e0aa9be0b1b4042e5ed79e577c9e'
      ]
    },
    bafkrei0002: {
      index: 1,
      proof: [
        '0x78163848a4d1f01159b7f72171adc0b9385fb847d30981b1938541573f937487',
        '0xc945d957e1d5e8ee0e3fe0b61fc6f5d567620853461020a1ecae426570945e6c',
        '0xcb431c434637e058084f23fc3798499aa631e0aa9be0b1b4042e5ed79e577c9e'
      ]
    },
    bafkrei0003: {
      index: 2,
      proof: [
        '0x702a687d525fca2c4181f1d244465e89a905ba8f9c36ad782876b29b69969da3',
        '0xdfe4c39071ee02142d8706d14247b57d6ec9988c7206c99650be9e541dde570a',
        '0xcb431c434637e058084f23fc3798499aa631e0aa9be0b1b4042e5ed79e577c9e'
      ]
    },
    bafkrei0004: {
      index: 3,
      proof: [
        '0x270b7b96c88ab7642c3617766312875857779c2e4cba8102073f00cce07173a3'
      ]
    },
    bafkrei0005: {
      index: 4,
      proof: [
        '0x0065219d7d9d36993b9c2df08908548ae9e0cd0b7fdf87415dd9ff35244d2cb9',
        '0xdfe4c39071ee02142d8706d14247b57d6ec9988c7206c99650be9e541dde570a',
        '0xcb431c434637e058084f23fc3798499aa631e0aa9be0b1b4042e5ed79e577c9e'
      ]
    }
  },
  repeated: {
    bafkrei0001: {
      index: 1,
      proof: [
        '0x78163848a4d1f01159b7f72171adc0b9385fb847d30981b1938541573f937487',
        '0x805afac11fb90858254651fff54514aeef375e6586d378f64f33c204ff574200'
      ]
    },
    bafkrei0002: {
      index: 2,
      proof: ['0xbe670e8b06d51338fc49c0bccdf00a14e010a4757da3cf284912b9d9c9f218fa']
    }
  }
}

describe('generateTree', () => {
  it.each(Object.entries(REFERENCE))('gives the reference root for %s', (_name, { input, root }) => {
    expect(generateTree([...input]).merkleRoot).toBe(root)
  })

  it.each(Object.entries(REFERENCE_PROOFS))('gives the reference proofs for %s', (name, proofs) => {
    expect(generateTree([...REFERENCE[name as keyof typeof REFERENCE_PROOFS].input]).proofs).toEqual(proofs)
  })

  it.each(Object.entries(REFERENCE))('proves each hash of %s against the root', (_name, { input }) => {
    const tree = generateTree([...input])
    for (const [contentHash, { index, proof }] of Object.entries(tree.proofs)) {
      expect(proof.reduce(hashPair, leafOf(index, contentHash))).toBe(tree.merkleRoot)
    }
  })

  it('keeps the order of its input', () => {
    const input = ['b', 'a']
    generateTree(input)
    expect(input).toEqual(['b', 'a'])
  })

  it('refuses an empty list', () => {
    expect(() => generateTree([])).toThrow('empty tree')
  })
})
