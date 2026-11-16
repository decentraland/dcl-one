import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const source = readFileSync(new URL('../src/start/landing_edit.js', import.meta.url), 'utf8')

function editor(initial = {}, fetch) {
  const handlers = new Map(), nodes = new Map(), requests = [], notices = []
  let focused, discard = true
  class Element {
    constructor(id = '') {
      this.id = id; this.dataset = {}; this.children = []; this.attributes = {}; this.disabled = false
      this.classList = { add() {}, toggle() {}, contains: name => (this.className || '').split(' ').includes(name) }
    }
    setAttribute(k, v) { this.attributes[k] = v }
    getAttribute(k) { return this.attributes[k] }
    removeAttribute(k) { delete this.attributes[k] }
    append(...children) { this.children.push(...children) }
    replaceChildren(...children) { this.children = children }
    addEventListener(name, fn) { this[name] = fn }
    querySelectorAll() { return descendants(this).filter(el => ['INPUT', 'BUTTON'].includes(el.tagName)) }
    focus() { focused = this }
    matches(selector) { return selector === '.lay__tab' && this.className === 'lay__tab' }
    closest(selector) {
      return selector === '.lay__tab' && this.matches(selector) || selector === '#lay-sadd' && this.id === 'lay-sadd' ||
        selector === '.lay__srow' && this.className === 'lay__srow' ? this : null
    }
    click() { return emit('click', this) }
  }
  const add = id => { const el = new Element(id); nodes.set(id, el); return el }
  const data = { tags: [], permissions: [], parcels: ['0,0'], base: '0,0', spawnPoints: [], ...initial }
  add('edit-data').textContent = JSON.stringify(data)
  add('media-hosts').value = ''
  const tabs = ['info', 'parcels', 'spawns', 'perms', 'server'].map(key => {
    const el = new Element(); el.dataset.laytab = key; el.className = 'lay__tab'; return el
  })
  const cells = ['0,0', '1,0'].map(key => { const el = new Element(); el.dataset.cell = key; return el })
  const descendants = el => [el, ...el.children.filter(c => typeof c === 'object').flatMap(descendants)]
  const document = {
    body: { classList: { add() {} } },
    getElementById: id => nodes.get(id) || [...nodes.values()].flatMap(descendants).find(el => el.id === id),
    querySelectorAll: selector => selector === '.lay__tab' ? tabs : selector.startsWith('#lay-sed') ? nodes.get('lay-sed').querySelectorAll() : [],
    querySelector: selector => selector.startsWith('.lay__cell') ? cells.find(c => selector.includes('"' + c.dataset.cell + '"')) : null,
    createElement: tag => { const el = new Element(); el.tagName = tag.toUpperCase(); return el },
    addEventListener: (name, fn) => handlers.set(name, [...(handlers.get(name) || []), fn]),
  }
  async function emit(name, target, extra = {}) {
    await Promise.all((handlers.get(name) || []).map(fn => fn({ target, preventDefault() {}, ...extra })))
  }
  if (data.grid) {
    const card = add('scene-layout')
    card.querySelectorAll = selector => selector === '.lay__tab' ? tabs : selector.includes('.lay__cell') ? cells : []
    card.querySelector = () => null
    add('lay-sed'); add('lay-sadd')
  }
  vm.runInNewContext(source, {
    document, URLSearchParams, pageToast: (...args) => notices.push(args), pageAuthHeaders: () => ({}), PAGE_OFFLINE: 'Offline',
    location: { search: '', pathname: '/scene', href: 'http://localhost/scene' },
    localStorage: { getItem() {}, setItem() {} }, window: { addEventListener() {} },
    confirm: () => discard,
    fetch: async (url, options) => {
      if (options.method === 'POST') { requests.push(JSON.parse(options.body)); return fetch ? fetch(requests.at(-1)) : { ok: true } }
      return { ok: false, text: async () => 'Refresh unavailable in fixture' }
    },
  })
  return {
    emit, nodes, tabs, cells, requests, notices, document,
    focused: () => focused, discard: value => { discard = value },
    permission(key) { const el = new Element(); el.dataset.perm = key; el.checked = true; el.className = 'sw'; return el },
    add,
  }
}

const tick = () => new Promise(resolve => setImmediate(resolve))

test('permission writes serialize and preserve successful earlier toggles', async () => {
  const pending = []
  const p = editor({}, () => new Promise(resolve => pending.push(resolve)))
  const first = p.permission('USE_WEB3_API'), second = p.permission('ALLOW_TO_TRIGGER_AVATAR_EMOTE')
  const a = p.emit('change', first), b = p.emit('change', second)
  await tick()
  assert.equal(p.requests.length, 1)
  assert.equal(first.disabled, true)
  pending.shift()({ ok: true }); await tick()
  assert.deepEqual(p.requests[1].requiredPermissions, ['USE_WEB3_API', 'ALLOW_TO_TRIGGER_AVATAR_EMOTE'])
  pending.shift()({ ok: true }); await Promise.all([a, b])
  assert.equal(second.disabled, false)
})

test('failed permission is rolled back before the queued change is calculated', async () => {
  let count = 0
  const p = editor({}, async () => ++count === 1 ? { ok: false, text: async () => 'Denied' } : { ok: true })
  const a = p.permission('USE_WEB3_API'), b = p.permission('USE_FETCH')
  await Promise.all([p.emit('change', a), p.emit('change', b)])
  assert.equal(a.checked, false)
  assert.deepEqual(p.requests[1].requiredPermissions, ['USE_FETCH'])
})

test('enabling media with no hosts leaves the switch off and focuses the hostname field', async () => {
  const p = editor(), permission = p.permission('ALLOW_MEDIA_HOSTNAMES')
  await p.emit('change', permission)
  assert.equal(permission.checked, false)
  assert.equal(p.requests.length, 0)
  assert.equal(p.focused(), p.nodes.get('media-hosts'))
})

test('comma edits preserve distinct tags that share the same space-joined text', async () => {
  const p = editor({ tags: ['a b'] })
  const input = { value: 'a, b', dataset: {}, classList: { contains: name => name === 'tags-input' }, closest: () => null }
  await p.emit('focusout', input)
  assert.deepEqual(p.requests[0], { tags: ['a', 'b'] })
})

test('scene tabs support arrows and Home/End with a single tab stop', async () => {
  const p = editor({ grid: { x0: -1, x1: 1, y0: -1, y1: 1 } })
  await p.emit('keydown', p.tabs[0], { key: 'ArrowLeft' })
  assert.equal(p.focused(), p.tabs[4])
  await p.emit('keydown', p.tabs[4], { key: 'Home' })
  assert.equal(p.focused(), p.tabs[0])
  await p.emit('keydown', p.tabs[0], { key: 'End' })
  assert.equal(p.focused(), p.tabs[4])
  assert.equal(p.tabs[4].tabIndex, 0)
  assert.equal(p.tabs[0].tabIndex, -1)
})


test('keyboard parcel editing protects the base and saves another parcel', async () => {
  const p = editor({ grid: { x0: -1, x1: 1, y0: -1, y1: 1 } })
  await p.emit('click', p.tabs[1])
  await p.emit('keydown', p.cells[0], { key: ' ' })
  assert.equal(p.requests.length, 0)
  await p.emit('keydown', p.cells[0], { key: 'ArrowRight' })
  assert.equal(p.focused(), p.cells[1])
  await p.emit('keydown', p.cells[1], { key: 'Enter' })
  assert.deepEqual(p.requests[0], { parcels: ['0,0', '1,0'] })
})

test('new spawn is editable without drawing, names its inputs and guards dirty replacement', async () => {
  const p = editor({ grid: { x0: -1, x1: 1, y0: -1, y1: 1 } })
  await p.emit('click', p.tabs[2])
  await p.emit('click', p.nodes.get('lay-sadd'))
  const name = p.document.getElementById('spawn-name')
  assert.equal(p.focused(), name)
  assert.equal(name.getAttribute('aria-label'), 'Spawn area name')
  assert.equal(p.document.getElementById('spawn-x1').value, 16)
  name.value = 'My arrival'; name.input()
  p.discard(false)
  await p.emit('click', p.nodes.get('lay-sadd'))
  assert.equal(p.document.getElementById('spawn-name').value, 'My arrival')
  const mount = p.nodes.get('lay-sed')
  const save = mount.children[0].children.at(-1).children[0]
  await save.click()
  assert.equal(p.requests[0].spawnPoints[0].name, 'My arrival')
})


test('pending spawn saves lock inputs and keep new drafts from replacing the submitted draft', async () => {
  let complete
  const p = editor({ grid: { x0: -1, x1: 1, y0: -1, y1: 1 } }, () => new Promise(resolve => { complete = resolve }))
  await p.emit('click', p.tabs[2])
  await p.emit('click', p.nodes.get('lay-sadd'))
  const name = p.document.getElementById('spawn-name')
  name.value = 'Arrival'; name.input()
  const buttons = p.nodes.get('lay-sed').children[0].children.at(-1).children
  const save = buttons[0].click()
  assert.equal(name.disabled, true)
  assert.equal(buttons.at(-1).disabled, true)
  await p.emit('click', p.nodes.get('lay-sadd'))
  assert.equal(p.document.getElementById('spawn-name').value, 'Arrival')
  complete({ ok: true }); await save
  assert.equal(p.focused(), p.nodes.get('lay-sadd'))
})

test('Cancel returns keyboard focus to Add spawn area', async () => {
  const p = editor({ grid: { x0: -1, x1: 1, y0: -1, y1: 1 } })
  await p.emit('click', p.tabs[2])
  await p.emit('click', p.nodes.get('lay-sadd'))
  await p.nodes.get('lay-sed').children[0].children.at(-1).children.at(-1).click()
  assert.equal(p.focused(), p.nodes.get('lay-sadd'))
})

test('an unreadable error body does not poison later permission saves', async () => {
  let count = 0
  const p = editor({}, async () => ++count === 1 ? { ok: false, text: async () => { throw new Error('Body failed') } } : { ok: true })
  await p.emit('change', p.permission('USE_WEB3_API'))
  await p.emit('change', p.permission('USE_FETCH'))
  assert.deepEqual(p.requests[1].requiredPermissions, ['USE_FETCH'])
  assert.equal(p.notices.some(([message]) => message === 'Body failed'), true)
})
