import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

const source = readFileSync(new URL('../src/start/storage.js', import.meta.url), 'utf8');
function harness(current, next = current, confirmation = false) {
  const listeners = {}, requests = [], notices = [], confirmations = [];
  const document = {
    getElementById: () => current,
    querySelector: () => null,
    createElement: () => ({ children: [], append(...children) { this.children.push(...children); } }),
    addEventListener: (name, handler) => { listeners[name] = handler; },
  };
  runInNewContext(source, {
    window: { addEventListener: (name, handler) => { listeners[name] = handler; } },
    document, location: { href: 'http://localhost/storage' }, encodeURIComponent,
    pageAuthHeaders: () => ({}), pageSyncHeader() {},
    parsePage: () => ({ getElementById: () => next }),
    pageToast: (...args) => notices.push(args),
    confirm: (message) => { confirmations.push(message); return confirmation; },
    fetch: async (url, init) => {
      requests.push({ url, ...init });
      return { ok: true, text: async () => '' };
    },
  });
  return { listeners, requests, notices, confirmations };
}
const section = () => ({ dataset: {}, contains: () => true, querySelector: () => null, querySelectorAll: () => [], replaceWith() {} });
const importForm = (merge) => ({
  elements: { file: { files: [{ text: async () => '{"world":{}}' }] }, merge: { checked: merge } },
  matches: (selector) => selector === '[data-import]',
  querySelectorAll: () => [], reset() {},
});
const submit = (h, target) => h.listeners.submit({ target, preventDefault() {} });

test('snapshot replacement can be cancelled before any request; merge needs no confirmation', async () => {
  const h = harness(section());
  await submit(h, importForm(false));
  assert.equal(h.requests.length, 0);
  assert.match(h.confirmations[0], /all local scene values, all players/);
  await submit(h, importForm(true));
  assert.equal(h.requests[0].url, '/storage/import?merge=1');
  assert.equal(h.confirmations.length, 1);
});

test('confirmed replacement uses the replacement endpoint', async () => {
  const h = harness(section(), undefined, true);
  await submit(h, importForm(false));
  assert.equal(h.requests[0].url, '/storage/import');
});

test('invalid snapshot keeps the form and reports the error without posting', async () => {
  const h = harness(section());
  const form = importForm(true);
  form.elements.file.files[0].text = async () => 'invalid';
  await submit(h, form);
  assert.equal(h.requests.length, 0);
  assert.equal(h.notices[0][1], true);
  assert.match(h.notices[0][0], /Choose a valid .json snapshot and try again/);
});

test('Cancel restores only its own row without fetching or replacing the section', async () => {
  let focused = false;
  const current = section(), original = { querySelector: () => ({ focus: () => { focused = true; } }) }, replacements = [];
  current.replaceWith = () => assert.fail('must keep the other drafts');
  const row = { _beforeEdit: original, replaceWith: (replacement) => replacements.push(replacement) };
  const button = { dataset: { act: 'cancel' }, closest: () => row };
  const h = harness(current);
  await h.listeners.click({ target: { closest: () => button } });
  assert.deepEqual(replacements, [original]);
  assert.equal(focused, true);
  assert.equal(h.requests.length, 0);
});

for (const present of [true, false]) test(`saving preserves drafts and forms; edited key remains listed: ${present}`, async () => {
  const current = section(), next = section(), moved = [];
  const pending = { dataset: { key: 'other' }, querySelector: () => ({ value: 'draft' }) };
  const saved = {
    dataset: { key: 'saved' },
    querySelector: () => ({ value: '42' }), querySelectorAll: () => [],
  };
  const add = { matches: () => false, elements: { key: { value: 'new' }, value: { value: 'draft' } } };
  const imported = { matches: () => true };
  current.querySelectorAll = (selector) => selector === '[data-key]' ? [pending, saved] : [add, imported];
  next.querySelectorAll = () => present ? [{ dataset: { key: 'other' }, replaceWith: (row) => moved.push(row) }] : [];
  next.querySelector = () => ({ replaceWith: (form) => moved.push(form), append: (wrapper) => moved.push(wrapper.children[1]) });
  const box = { dataset: { scope: 'scene' } };
  const button = { dataset: { act: 'save' }, closest: (selector) => selector === '[data-key]' ? saved : box };
  const h = harness(current, next);
  await h.listeners.click({ target: { closest: () => button } });
  assert.equal(h.requests[0].method, 'PUT');
  assert.equal(h.requests[0].body, '{"value":42}');
  assert.deepEqual(moved, [pending, add, imported]);
});


test('navigation warns about edited storage and unfinished add drafts', () => {
  const current = section();
  current.querySelectorAll = () => [{ dataset: { json: '42' }, querySelector: () => ({ value: '43' }) }];
  const h = harness(current);
  let warned = false;
  h.listeners.beforeunload({ preventDefault: () => { warned = true; } });
  assert.equal(warned, true);
});
