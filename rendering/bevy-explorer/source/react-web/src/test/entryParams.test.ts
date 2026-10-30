// DOMAIN: the entry url's accepted-parameter set (lib/entryParams.ts) and the dialog that names
// what a link carried that nothing reads (features/gate/EntryParamsDialog.tsx).

import { describe, expect, it } from 'vitest'
import { WEB_PARAMS } from '../engine/generated'
import { BENCHMARK_PARAMS } from '../lib/bootMode'
import { acceptedEntryParams, entryRealm, perfStreamPath, unrecognisedEntryParams } from '../lib/entryParams'

describe('acceptedEntryParams', () => {
  it('is every link-settable engine param plus the page switches, with a doc each', () => {
    const accepted = acceptedEntryParams()
    const names = accepted.map((p) => p.name)
    // the benchmark's switches are no link's to set (lib/bootMode.ts), so they are not advertised
    for (const p of WEB_PARAMS) expect(names.includes(p.name)).toBe(p.delivery !== 'host' && !BENCHMARK_PARAMS.includes(p.name))
    for (const p of accepted) expect(p.doc.length).toBeGreaterThan(0)
    expect(new Set(names).size).toBe(names.length)
    expect(names).toContain('guest')
    expect(names).toContain('fps')
    expect(names).toContain('pulseCertificateHash')
    // host-forced and debug switches are known, not advertised
    for (const internal of ['native', 'hud', 'mock', 'gate', 'simerror']) expect(names).not.toContain(internal)
  })
})

describe('unrecognisedEntryParams', () => {
  it('is empty for a link made of accepted params only', () => {
    expect(unrecognisedEntryParams(new URLSearchParams('?realm=x&position=1,2&mock=1&preview&world=boedo'))).toEqual([])
    expect(unrecognisedEntryParams(new URLSearchParams('?native=1&hud=0&gate=gpu&simerror=realm'))).toEqual([])
    // campaign links: Segment reads them, never called out
    expect(unrecognisedEntryParams(new URLSearchParams('?utm_source=x&utm_campaign=y&anon_user_id=abc&analyticsSession=s'))).toEqual([])
    // ad clicks: a Google Ads final url as the ad serves it, and other platforms' click ids
    expect(unrecognisedEntryParams(new URLSearchParams('?gad_source=1&gad_campaignid=2231&gbraid=0AAAAA_KoQHo8&gclid=Cj0K'))).toEqual([])
    expect(unrecognisedEntryParams(new URLSearchParams('?fbclid=a&msclkid=b&ttclid=c&twclid=d&li_fat_id=e&rdt_cid=f'))).toEqual([])
  })

  it('knows the benchmark switches only where the page acts on them (bare or empty, the shell hands the flag on as `autopilot=`)', () => {
    // never advertised: no link may set them
    for (const name of BENCHMARK_PARAMS) expect(acceptedEntryParams().map((p) => p.name)).not.toContain(name)
    for (const search of ['?autopilot&realm=supportteam.dcl.eth', '?autopilot=&realm=supportteam.dcl.eth']) {
      // where the page honours them (loopback, native) App names them as the page's own: nothing to call out
      expect(unrecognisedEntryParams(new URLSearchParams(search), BENCHMARK_PARAMS)).toEqual([])
      // anywhere else they are ignored, and a link carrying them is told so
      expect(unrecognisedEntryParams(new URLSearchParams(search))).toEqual(['autopilot'])
    }
    const stream = new URLSearchParams('?perfStream=/__perf/change_build&perfCsv')
    expect(unrecognisedEntryParams(stream, BENCHMARK_PARAMS)).toEqual([])
    expect(unrecognisedEntryParams(stream)).toEqual(['perfStream', 'perfCsv'])
  })

  it('names each unknown key once, including the host-only editor flag', () => {
    const q = new URLSearchParams('?realm=x&reaml=y&editor&reaml=z&initialRealm=w')
    expect(unrecognisedEntryParams(q)).toEqual(['reaml', 'editor', 'initialRealm'])
  })
})

describe('entryRealm', () => {
  it('reads a world name as its .dcl.eth realm, with realm taking precedence', () => {
    expect(entryRealm(new URLSearchParams('?world=boedo'))).toBe('boedo.dcl.eth')
    expect(entryRealm(new URLSearchParams('?world=Boedo.dcl.eth'))).toBe('boedo.dcl.eth')
    expect(entryRealm(new URLSearchParams('?world=boedo&realm=https://r'))).toBe('https://r')
    expect(entryRealm(new URLSearchParams('?world=https://evil.example'))).toBeNull()
    expect(entryRealm(new URLSearchParams('?position=1,2'))).toBeNull()
  })
})

describe('perfStreamPath', () => {
  it('is a path on this origin and nothing else', () => {
    const path = (search: string) => perfStreamPath(new URLSearchParams(search))
    expect(path('?perfStream=/__perf/change_build_1')).toBe('/__perf/change_build_1')
    expect(path('?perfStream=/__perf/b/')).toBe('/__perf/b')
    for (const elsewhere of [
      'https://evil.example/x',
      '//evil.example/x',
      '/\\evil.example',
      'perf/b',
      '/a b',
      '/a?b=c',
      '/a#b',
      '/',
      '',
      'javascript:alert(1)'
    ]) {
      expect(path('?perfStream=' + encodeURIComponent(elsewhere))).toBeNull()
    }
    expect(path('?perfCsv')).toBeNull()
  })
})
