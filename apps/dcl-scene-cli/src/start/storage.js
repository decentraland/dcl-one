(() => {
  const SOURCE = { 'x-dcl-one-storage-source': 'ui' };
  const section = () => document.getElementById('storage');
  const prefix = () => (section() && section().dataset.prefix) || '';
  const encode = encodeURIComponent;
  const urlFor = (scope, address, key) => {
    if (scope === 'player') return `${prefix()}/players/${encode(address)}/values/${encode(key)}`;
    if (scope === 'env') return `${prefix()}/env/${encode(key)}`;
    return `${prefix()}/values/${encode(key)}`;
  };
  const scopeUrl = (scope, address) => {
    if (scope === 'player') return `${prefix()}/players/${encode(address)}/values`;
    if (scope === 'env') return `${prefix()}/env`;
    return `${prefix()}/values`;
  };
  const failure = async (res) => {
    let why = `HTTP ${res.status}`;
    try {
      const text = await res.text();
      try {
        const body = JSON.parse(text);
        if (body && body.message) why = body.message;
      } catch {
        if (text.trim()) why = text.trim();
      }
    } catch {}
    return new Error(why);
  };
  // Every call carries the run's token: writes need it on top of the
  // origin gate, and a redraw fetched with it comes back with the tokens
  // the page's forms post.
  const call = async (url, init) => {
    let res;
    try {
      res = await fetch(url, { ...(init || {}), headers: { ...((init && init.headers) || {}), ...pageAuthHeaders() } });
    } catch {
      throw new Error(PAGE_OFFLINE);
    }
    if (!res.ok) throw await failure(res);
    return res;
  };
  const swap = (html, preserveDrafts = false, savedRow) => {
    const doc = parsePage(html);
    const next = doc.getElementById('storage');
    const current = section();
    const focused = document.activeElement;
    if (next && current) {
      if (preserveDrafts) {
        for (const row of current.querySelectorAll('[data-key]')) {
          if (row === savedRow || !row.querySelector('.sto__edit')) continue;
          const replacement = [...next.querySelectorAll('[data-key]')].find((r) => r.dataset.key === row.dataset.key);
          if (replacement) replacement.replaceWith(row);
          else {
            const pending = document.createElement('div');
            pending.className = 'sto__pending';
            const note = document.createElement('p');
            note.className = 'note';
            note.textContent = `Unsaved draft for ${row.dataset.key}; this key is absent from the current page. Save to write it, or Cancel to discard it.`;
            pending.append(note, row);
            next.querySelector('[data-scope]')?.append(pending);
          }
        }
        for (const form of current.querySelectorAll('[data-add], [data-import]')) {
          if (form.matches('[data-import]') || form.elements.key.value || form.elements.value.value) {
            next.querySelector(form.matches('[data-import]') ? '[data-import]' : '[data-add]')?.replaceWith(form);
          }
        }
      }
      current.replaceWith(next);
      if (focused?.isConnected) focused.focus();
      else if (savedRow) [...next.querySelectorAll('[data-key]')]
        .find((row) => row.dataset.key === savedRow.dataset.key)?.querySelector('[data-act="edit"]')?.focus();
    }
    pageSyncHeader(doc);
    syncTargetForm();
  };
  const refresh = async (savedRow) => {
    const res = await call(location.href, { headers: { accept: 'text/html' } });
    swap(await res.text(), true, savedRow);
  };
  const parseValue = (text, kind) => {
    if (kind === 'env') return text;
    const trimmed = text.trim();
    if (!trimmed) return '';
    try {
      return JSON.parse(trimmed);
    } catch {
      return text;
    }
  };
  const put = (scope, address, key, value) =>
    call(urlFor(scope, address, key), {
      method: 'PUT',
      headers: { ...SOURCE, 'content-type': 'application/json' },
      body: JSON.stringify({ value }),
    });
  const remove = (scope, address, key) => call(urlFor(scope, address, key), { method: 'DELETE', headers: SOURCE });
  const clearAll = (scope, address) =>
    call(scopeUrl(scope, address), { method: 'DELETE', headers: { ...SOURCE, 'x-confirm-delete-all': 'true' } });
  const scopeOf = (el) => {
    const box = el.closest('[data-scope]');
    return { scope: box.dataset.scope, address: box.dataset.address || '', kind: box.dataset.kind || 'json' };
  };
  const busy = (el, on) => {
    for (const b of el.querySelectorAll('button')) b.disabled = on;
  };
  const startEdit = (row) => {
    const val = row.querySelector('.sto__val');
    if (!val || row.querySelector('.sto__edit')) return;
    row._beforeEdit = row.cloneNode(true);
    const area = document.createElement('textarea');
    area.className = 'sto__edit';
    area.value = row.dataset.json;
    area.setAttribute('aria-label', `Value of ${row.dataset.key}`);
    val.replaceWith(area);
    row.querySelector('.sto__acts').innerHTML =
      '<button class="sto__btn" type="button" data-act="save">Save</button><button class="sto__btn" type="button" data-act="cancel">Cancel</button>';
    area.focus();
  };
  document.addEventListener('click', async (event) => {
    const btn = event.target.closest('[data-act]');
    if (!btn || !section() || !section().contains(btn)) return;
    const act = btn.dataset.act;
    const row = btn.closest('[data-key]');
    try {
      if (act === 'edit') return startEdit(row);
      if (act === 'cancel') {
        const pending = row.closest?.('.sto__pending');
        if (pending) pending.remove();
        else {
          row.replaceWith(row._beforeEdit);
          row._beforeEdit.querySelector('[data-act="edit"]')?.focus();
        }
        return;
      }
      if (act === 'reveal') {
        row.querySelector('.sto__val').textContent = row.dataset.json;
        btn.remove();
        return;
      }
      const { scope, address, kind } = scopeOf(btn);
      if (act === 'save') {
        const area = row.querySelector('.sto__edit');
        busy(row, true);
        await put(scope, address, row.dataset.key, parseValue(area.value, kind));
        pageToast(`Saved ${row.dataset.key}`);
        return await refresh(row);
      }
      if (act === 'delete') {
        if (!confirm(`Delete ${row.dataset.key}?`)) return;
        busy(row, true);
        await remove(scope, address, row.dataset.key);
        pageToast(`Deleted ${row.dataset.key}`);
        return await refresh();
      }
      if (act === 'clear') {
        const what = btn.dataset.what || 'every value';
        if (!confirm(`Delete ${what}? This cannot be undone.`)) return;
        btn.disabled = true;
        await clearAll(scope, address);
        pageToast(`Cleared ${what}`);
        return await refresh();
      }
    } catch (e) {
      pageToast(e.message || String(e), true);
      if (row) busy(row, false);
      btn.disabled = false;
    }
  });
  document.addEventListener('submit', async (event) => {
    const form = event.target;
    if (!section() || !section().contains(form)) return;
    if (form.matches('[data-add]')) {
      event.preventDefault();
      const { scope, address } = scopeOf(form);
      const kind = form.dataset.kind || 'json';
      const key = form.elements.key.value.trim();
      if (!key) return pageToast('A key is required', true);
      busy(form, true);
      try {
        await put(scope, address, key, parseValue(form.elements.value.value, kind));
        pageToast(`Saved ${key}`);
        form.reset();
        busy(form, false);
        await refresh();
      } catch (e) {
        pageToast(e.message, true);
        busy(form, false);
      }
      return;
    }
    if (form.matches('[data-target-form]')) {
      event.preventDefault();
      busy(form, true);
      try {
        const res = await call(form.action, {
          method: 'POST',
          headers: { 'content-type': 'application/x-www-form-urlencoded' },
          body: new URLSearchParams(new FormData(form)),
        });
        swap(await res.text());
        pageToast('Storage target saved');
      } catch (e) {
        pageToast(e.message, true);
        busy(form, false);
      }
      return;
    }
    if (form.matches('[data-import]')) {
      event.preventDefault();
      const file = form.elements.file.files[0];
      if (!file) return pageToast('Choose a snapshot file first', true);
      busy(form, true);
      try {
        const text = await file.text();
        try { JSON.parse(text); }
        catch { throw new Error('Snapshot is not valid JSON. Choose a valid .json snapshot and try again.'); }
        const merge = form.elements.merge.checked;
        if (!merge && !confirm('Replace all local scene values, all players’ values and runtime environment overrides with this snapshot? Values absent from the snapshot will be deleted. This cannot be undone.')) {
          busy(form, false);
          return;
        }
        await call(`${prefix()}/storage/import${merge ? '?merge=1' : ''}`, {
          method: 'POST',
          headers: { ...SOURCE, 'content-type': 'application/json' },
          body: text,
        });
        pageToast(merge ? 'Merged the snapshot in' : 'Replaced storage with the snapshot');
        form.reset();
        busy(form, false);
        await refresh();
      } catch (e) {
        pageToast(e.message, true);
        busy(form, false);
      }
    }
  });
  const syncTargetForm = () => {
    const form = document.querySelector('[data-target-form]');
    if (!form) return;
    const upstream = form.querySelector('input[name="upstream"]');
    const choice = form.querySelector('[data-choice]');
    const url = form.querySelector('input[name="url"]');
    const custom = form.querySelector('input[name="service"][value="custom"]');
    if (choice) choice.hidden = !upstream.checked;
    if (url) url.hidden = !(upstream.checked && custom && custom.checked);
  };
  document.addEventListener('change', (event) => {
    if (event.target.closest('[data-target-form]')) syncTargetForm();
  });
  window.addEventListener('beforeunload', (event) => {
    const drafts = [...(section()?.querySelectorAll('[data-key]') || [])].some((row) => {
      const area = row.querySelector('.sto__edit');
      return area && area.value !== row.dataset.json;
    });
    const add = section()?.querySelector('[data-add]');
    if (drafts || (add && (add.elements.key.value || add.elements.value.value))) {
      event.preventDefault();
      event.returnValue = '';
    }
  });
  syncTargetForm();
})();
