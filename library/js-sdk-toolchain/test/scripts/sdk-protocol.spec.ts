import { withoutHiddenBlocks } from '../../scripts/protocol-buffer-generation/sdkProtocol'

describe('the protocol the SDK is generated from', () => {
  it('should drop what a hidden block holds and keep the rest', () => {
    const proto = [
      'message PBThing {',
      '  int32 shown = 1;',
      '',
      '  // sdk:hidden-begin',
      '',
      '  int32 hidden = 2;',
      '',
      '  // sdk:hidden-end',
      '',
      '  int32 also_shown = 3;',
      '}'
    ].join('\n')

    const result = withoutHiddenBlocks(proto, 'thing.proto')

    expect(result).toContain('int32 shown = 1;')
    expect(result).toContain('int32 also_shown = 3;')
    expect(result).not.toContain('hidden')
  })

  it('should refuse a block that is never closed, so a typo cannot hide the rest of a file', () => {
    expect(() => withoutHiddenBlocks('// sdk:hidden-begin\nint32 a = 1;', 'thing.proto')).toThrow('never closed')
  })

  it('should refuse an end marker with no begin, so a typo cannot publish a hidden field', () => {
    expect(() => withoutHiddenBlocks('int32 a = 1;\n// sdk:hidden-end', 'thing.proto')).toThrow('without a')
  })
})
