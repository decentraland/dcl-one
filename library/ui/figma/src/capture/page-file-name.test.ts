import { describe, expect, it } from 'vitest'
import { decodePageFileName, encodePageFileName, isWindowsSafeName } from './page-file-name.js'

describe('when naming the capture file of a Figma page', () => {
  it('should keep an ordinary page name as it is', () => {
    expect(encodePageFileName('Chat & Voice Chat')).toBe('Chat & Voice Chat.json')
  })

  it('should encode the colon of the legacy Bars page and read it back', () => {
    const name = 'Bars: Nav bar - Tabs bar - Footer'
    expect(encodePageFileName(name)).toBe('Bars%3A Nav bar - Tabs bar - Footer.json')
    expect(decodePageFileName(encodePageFileName(name))).toBe(name)
  })

  it.each([
    [
      'every character Windows rejects',
      'a<b>c"d/e\\f|g?h*i',
      'a%3Cb%3Ec%22d%2Fe%5Cf%7Cg%3Fh%2Ai.json'
    ],
    ['a control character', 'tab\there', 'tab%09here.json'],
    ['the delete character', 'del\u007fete', 'del%7Fete.json'],
    ['a trailing dot', 'Draft.', 'Draft%2E.json'],
    ['a trailing space', 'Draft ', 'Draft%20.json'],
    ['a device stem', 'CON', '%43ON.json'],
    ['a device stem in another case', 'Con', '%43on.json'],
    ['a device stem with an extension', 'con.txt', '%63on.txt.json'],
    ['a device stem that already ends in .json', 'con.json', '%63on.json.json'],
    ['a numbered device stem', 'COM1.txt', '%43OM1.txt.json'],
    ['a superscript device stem', 'com¹', '%63om¹.json'],
    ['a superscript device stem with an extension', 'LPT².txt', '%4CPT².txt.json'],
    ['a superscript device stem in another case', 'Com³', '%43om³.json'],
    ['the superscript COM¹', 'COM¹', '%43OM¹.json'],
    ['the superscript com² ending in .json', 'com².json', '%63om².json.json'],
    ['the superscript LPT³ with an extension', 'LPT³.txt', '%4CPT³.txt.json'],
    ['a literal percent sign', '100% done', '100%25 done.json'],
    ['text that looks encoded', 'a%3Ab', 'a%253Ab.json'],
    ['an encoded percent sign', '%25', '%2525.json'],
    ['non-ASCII text', 'Café ñ 日本', 'Café ñ 日本.json']
  ])('should encode %s, read it back, and stay valid on Windows', (_, name, file) => {
    expect(encodePageFileName(name)).toBe(file)
    expect(isWindowsSafeName(file)).toBe(true)
    expect(decodePageFileName(file)).toBe(name)
  })

  it('should not treat a name that only starts like a device as reserved', () => {
    expect(encodePageFileName('Console')).toBe('Console.json')
    expect(encodePageFileName('COM10')).toBe('COM10.json')
  })

  it('should keep every device-stem variant distinct, reversible and valid on Windows', () => {
    const stems = ['com', 'COM', 'Com', 'lpt', 'LPT', 'Lpt', 'con', 'AUX', 'nul', 'Prn']
    const suffixes = ['', '1', '9', '10', '¹', '²', '³']
    const tails = ['', '.txt', '.json', '.', ' ', '.¹']
    const names = stems.flatMap((stem) =>
      suffixes.flatMap((suffix) => tails.map((tail) => stem + suffix + tail))
    )
    const files = names.map(encodePageFileName)
    expect(names.filter((_, index) => !isWindowsSafeName(files[index]!))).toEqual([])
    expect(names.filter((name, index) => decodePageFileName(files[index]!) !== name)).toEqual([])
    expect(new Set(files).size).toBe(names.length)
  })

  it('should give names that differ only in a reserved character different files', () => {
    const names = ['a:b', 'a?b', 'a*b', 'a%3Ab', 'a%3Fb', 'a b', 'a.b']
    const files = names.map(encodePageFileName)
    expect(new Set(files).size).toBe(names.length)
  })

  it('should be injective and reversible over every short name from a hostile alphabet', () => {
    const alphabet = ['a', 'C', 'o', 'N', '1', '.', ' ', '%', '3', 'A', ':', '?', '\u007f', 'é']
    const names = ['']
    for (let length = 1; length <= 4; length++)
      for (const prefix of names.filter((name) => name.length === length - 1))
        for (const character of alphabet) names.push(prefix + character)
    const files = names.map(encodePageFileName)
    expect(names.length).toBe(1 + 14 + 14 ** 2 + 14 ** 3 + 14 ** 4)
    expect(names.filter((_, index) => !isWindowsSafeName(files[index]!))).toEqual([])
    expect(names.filter((name, index) => decodePageFileName(files[index]!) !== name)).toEqual([])
    expect(new Set(files).size).toBe(names.length)
  })
})

describe('when checking a single path name for a Windows checkout', () => {
  it.each([
    ['a reserved character', 'a:b.json'],
    ['a control character', 'a\u0001b.json'],
    ['the delete character', 'a\u007fb.json'],
    ['a trailing dot', 'folder.'],
    ['a trailing space', 'folder '],
    ['a device stem', 'aux.json'],
    ['a device stem in another case', 'Lpt3'],
    ['a superscript device stem', 'COM³.json'],
    ['a superscript device stem in another case with an extension', 'lpt¹.txt']
  ])('should reject %s', (_, name) => {
    expect(isWindowsSafeName(name)).toBe(false)
  })
})
