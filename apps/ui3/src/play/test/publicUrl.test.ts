// The page directory the engine, bridge scene and service worker resolve against.
import { describe, expect, it } from 'vitest'
import { pageDir } from '../lib/publicUrl'

describe('pageDir', () => {
  it.each([
    ['http://127.0.0.1:5212/play/app?mock=1', 'http://127.0.0.1:5212/play/'],
    ['http://127.0.0.1:5212/play/app/?mock=1', 'http://127.0.0.1:5212/play/'],
    ['http://127.0.0.1:5212/play/app.html?mock=1', 'http://127.0.0.1:5212/play/'],
    ['http://127.0.0.1:5212/play/app.html/?mock=1', 'http://127.0.0.1:5212/play/'],
    ['https://cdn.example/web/app.html', 'https://cdn.example/web/'],
    ['https://decentraland.zone/bevy-web', 'https://decentraland.zone/bevy-web/'],
    ['https://decentraland.zone/bevy-web/', 'https://decentraland.zone/bevy-web/'],
    ['http://127.0.0.1:5212/play', 'http://127.0.0.1:5212/play/'],
  ])('%s resolves to %s', (href, dir) => {
    expect(pageDir(href)).toBe(dir)
  })
})
