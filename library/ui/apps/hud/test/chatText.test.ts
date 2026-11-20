import { describe, it, expect } from 'vitest'
import { parseMessage, buildNameIndex, mentionsMe } from '@explorer-core/features/chat/chatParse'

describe('parseMessage', () => {
  it('keeps plain text as a single token', () => {
    expect(parseMessage('hello world')).toEqual([{ type: 'text', value: 'hello world' }])
  })

  it('extracts a URL in order', () => {
    expect(parseMessage('see https://dcl.org/x ok')).toEqual([
      { type: 'text', value: 'see ' },
      { type: 'url', value: 'https://dcl.org/x' },
      { type: 'text', value: ' ok' }
    ])
  })

  it('extracts a location coord with parsed x/y', () => {
    expect(parseMessage('go 10,-5 now')).toEqual([
      { type: 'text', value: 'go ' },
      { type: 'location', value: '10,-5', x: 10, y: -5 },
      { type: 'text', value: ' now' }
    ])
  })

  it('extracts an @mention (with optional #tag)', () => {
    expect(parseMessage('hi @Alice!')).toEqual([
      { type: 'text', value: 'hi ' },
      { type: 'mention', value: '@Alice', name: 'Alice', tag: undefined },
      { type: 'text', value: '!' }
    ])
    expect(parseMessage('yo @Bob#12ab')[1]).toEqual({ type: 'mention', value: '@Bob#12ab', name: 'Bob', tag: '12ab' })
  })

  it('extracts a world name (.dcl.eth and bare .eth)', () => {
    expect(parseMessage('join boedo.dcl.eth now')).toEqual([
      { type: 'text', value: 'join ' },
      { type: 'world', value: 'boedo.dcl.eth' },
      { type: 'text', value: ' now' }
    ])
    expect(parseMessage('go name.eth')[1]).toEqual({ type: 'world', value: 'name.eth' })
  })

  it('does not break a URL that contains a world name', () => {
    expect(parseMessage('https://play.decentraland.org/?realm=boedo.dcl.eth')).toEqual([
      { type: 'url', value: 'https://play.decentraland.org/?realm=boedo.dcl.eth' }
    ])
  })

  it('reads a URL scheme and a world suffix in either case', () => {
    expect(parseMessage('HTTPS://Dcl.org/X')).toEqual([{ type: 'url', value: 'HTTPS://Dcl.org/X' }])
    expect(parseMessage('Http://dcl.org')).toEqual([{ type: 'url', value: 'Http://dcl.org' }])
    expect(parseMessage('Boedo.DCL.ETH')).toEqual([{ type: 'world', value: 'Boedo.DCL.ETH' }])
    expect(parseMessage('Name.Eth')).toEqual([{ type: 'world', value: 'Name.Eth' }])
  })

  it('handles a message mixing all three', () => {
    const t = parseMessage('@Alice come to 5,5 see https://x.io')
    expect(t.map((x) => x.type)).toEqual(['mention', 'text', 'location', 'text', 'url'])
  })
})

describe('bold and italic tags', () => {
  const tag = (value: string, style: 'b' | 'i', close = false) => ({ type: 'tag', value, style, close })

  it('reads <b> and <i>, opening and closing', () => {
    expect(parseMessage('This should not: <b>bold</b>')).toEqual([
      { type: 'text', value: 'This should not: ' },
      tag('<b>', 'b'),
      { type: 'text', value: 'bold' },
      tag('</b>', 'b', true)
    ])
    expect(parseMessage('<i>italic</i>').map((t) => t.type)).toEqual(['tag', 'text', 'tag'])
    // anything after whitespace up to the `>`
    expect(parseMessage('<b x=1>a</b\ty>')).toEqual([tag('<b x=1>', 'b'), { type: 'text', value: 'a' }, tag('</b\ty>', 'b', true)])
  })

  it('a tag ends at its own >, within reach', () => {
    // no further than another tag's start
    expect(parseMessage('x <b hello <i>world')).toEqual([{ type: 'text', value: 'x <b hello ' }, tag('<i>', 'i'), { type: 'text', value: 'world' }])
    const long = `<b ${'x'.repeat(33)}>`
    expect(parseMessage(long)).toEqual([{ type: 'text', value: long }])
    const fits = `<b ${'x'.repeat(32)}>`
    expect(parseMessage(`${fits}a`)).toEqual([tag(fits, 'b'), { type: 'text', value: 'a' }])
    // many openings with no end are read once each
    const flood = '<b '.repeat(20000)
    const start = performance.now()
    expect(parseMessage(flood)).toEqual([{ type: 'text', value: flood }])
    expect(performance.now() - start).toBeLessThan(500)
  })

  it('leaves every other tag as text', () => {
    for (const text of ['<B>caps</B>', '<I>', 'This should be escaped: <br>', 'Multiple <br> and <hr>', "<font color='red'>", '<bold>', '< b>', '<>', 'a < b', '<b', '<u>x</u>']) {
      expect(parseMessage(text)).toEqual([{ type: 'text', value: text }])
    }
  })

  it('a tag that styles no text is text itself', () => {
    // an empty pair, a closing tag with nothing open, an opening one with nothing after
    for (const text of ['<b></b>', 'a</i>b', 'a <b>', '<b><i></b></i>', '<b><i></i></b>', '</b >']) {
      expect(parseMessage(text)).toEqual([{ type: 'text', value: text }])
    }
    // so no hidden tag parts a link from the text beside it
    expect(parseMessage('boe<i></i>do.dcl.eth')).toEqual([
      { type: 'text', value: 'boe<i></i>' },
      { type: 'world', value: 'do.dcl.eth' }
    ])
    expect(parseMessage('https://dcl.org/</b>https://x.org')).toEqual([
      { type: 'url', value: 'https://dcl.org/' },
      { type: 'text', value: '</b>' },
      { type: 'url', value: 'https://x.org' }
    ])
    // a second opening inside the first, and the closing left over
    expect(parseMessage('<b>a<b>b</b>c</b>')).toEqual([tag('<b>', 'b'), { type: 'text', value: 'a<b>b' }, tag('</b>', 'b', true), { type: 'text', value: 'c</b>' }])
    // a tag of the other style between is no text
    expect(parseMessage('<b><i>a</i></b>').map((t) => t.type)).toEqual(['tag', 'tag', 'text', 'tag', 'tag'])
    // whitespace is text: the pair is hidden around it
    expect(parseMessage('<b> </b>')).toEqual([tag('<b>', 'b'), { type: 'text', value: ' ' }, tag('</b>', 'b', true)])
    // a closing tag past the one that ended the style
    expect(parseMessage('<b>x</b></b>')).toEqual([tag('<b>', 'b'), { type: 'text', value: 'x' }, tag('</b>', 'b', true), { type: 'text', value: '</b>' }])
    // each style ends at its own closing tag, in whichever order they come
    expect(parseMessage('<b><i>x</b></i>')).toEqual([tag('<b>', 'b'), tag('<i>', 'i'), { type: 'text', value: 'x' }, tag('</b>', 'b', true), tag('</i>', 'i', true)])
    // many tags are read once each
    const flood = '<b><i>'.repeat(20000)
    const start = performance.now()
    expect(parseMessage(flood)).toEqual([{ type: 'text', value: flood }])
    expect(performance.now() - start).toBeLessThan(500)
  })

  it('reads links between tags once a space parts them, and a URL up to a tag', () => {
    expect(parseMessage('<b> @Alice </b>').map((t) => t.type)).toEqual(['tag', 'text', 'mention', 'text', 'tag'])
    // a tag ends a word as a space does: a mention may stand right inside one
    expect(parseMessage('<b>@Alice</b>')).toEqual([tag('<b>', 'b'), { type: 'mention', value: '@Alice', name: 'Alice', tag: undefined }, tag('</b>', 'b', true)])
    expect(parseMessage('<i>hi @Bob#12ab</i>').map((t) => t.type)).toEqual(['tag', 'text', 'mention', 'tag'])
    // a tag with more in its brackets is one too
    expect(parseMessage('<b x>@Alice').map((t) => t.type)).toEqual(['tag', 'mention'])
    // any other tag is text, and no boundary: an uppercase one among them
    expect(parseMessage('<u>@Alice</u>')).toEqual([{ type: 'text', value: '<u>@Alice</u>' }])
    expect(parseMessage('<B>@Alice</B>')).toEqual([{ type: 'text', value: '<B>@Alice</B>' }])
    expect(parseMessage('mail@Alice')).toEqual([{ type: 'text', value: 'mail@Alice' }])
    expect(parseMessage('<b>https://dcl.org/x</b>')).toEqual([tag('<b>', 'b'), { type: 'url', value: 'https://dcl.org/x' }, tag('</b>', 'b', true)])
    expect(parseMessage('<i>10,20</i>')[1]).toEqual({ type: 'location', value: '10,20', x: 10, y: 20 })
  })
})

describe('buildNameIndex + mentionsMe', () => {
  const ALICE = '0x00000000000000000000000000000000000a1b2c'
  const members = [
    { address: '0x00000000000000000000000000000000000000aa', name: 'Me', claimed: true },
    { address: ALICE, name: 'Alice', claimed: false }
  ]

  it('indexes an unclaimed name with its wallet tag, and bare while unambiguous', () => {
    const idx = buildNameIndex(members)
    expect(idx.get('me')).toBe(members[0].address)
    expect(idx.get('alice#1b2c')).toBe(ALICE)
    expect(idx.get('alice')).toBe(ALICE)
  })

  it('a claimed name is mentioned by name alone', () => {
    const me = { address: '0x00000000000000000000000000000000000000aa', name: 'Me', hasClaimedName: true }
    expect(mentionsMe('hey @Me!', me)).toBe(true)
    // what renders as a mention is what counts as one
    for (const text of ['<b>@Me</b>', '<b x>@Me', 'hi<i>@Me</i >there']) {
      expect(mentionsMe(text, me)).toBe(true)
      expect(parseMessage(text).some((t) => t.type === 'mention')).toBe(true)
    }
    // text in a tag's brackets is hidden, and no mention; the space a tag counts as completes no
    // other tag
    for (const text of ['<u>@Me</u>', '<B>@Me</B>', '<I>@Me', '<b @Me x>hello', 'hey <i @Me>', '<b @Me x>', '<b<i>>@Me', '@Me<b <i>>x']) {
      expect(mentionsMe(text, me)).toBe(false)
      expect(parseMessage(text).some((t) => t.type === 'mention')).toBe(false)
    }
    expect(mentionsMe('hey @Alice', me)).toBe(false)
  })

  it('an unclaimed name is mentioned only with its own wallet tag', () => {
    const me = { address: ALICE, name: 'Alice#1b2c', hasClaimedName: false }
    expect(mentionsMe('hi @Alice#1b2c', me)).toBe(true)
    expect(mentionsMe('hi @Alice#ffff', me)).toBe(false)
  })
})
