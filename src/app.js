// XPAD Small Deck — frontend logic.
// Talks to the Rust backend via Tauri's invoke; file picking via the dialog plugin.

const TAURI = window.__TAURI__ || {};
const invoke = TAURI.core ? TAURI.core.invoke : () => Promise.reject('Tauri not available');

// Tag <html> with the OS so CSS can adapt the window chrome: macOS uses an
// integrated title bar (traffic lights over our strip); Windows/Linux keep
// their native title bar, so we hide our strip there.
(function () {
  var ua = navigator.userAgent;
  var os = /Mac/i.test(ua) ? 'mac' : /Windows/i.test(ua) ? 'win' : 'linux';
  document.documentElement.classList.add('os-' + os);
})();

// Open a native file/folder picker via the dialog plugin's command channel
// directly, so we don't need to bundle the plugin's JS package (buildless UI).
function openFileDialog(title, directory) {
  return invoke('plugin:dialog|open', {
    options: { multiple: false, directory: !!directory, title }
  });
}

const ALL_FKEYS = ['F13', 'F14', 'F15', 'F16', 'F17', 'F18', 'F19', 'F20', 'F21', 'F22', 'F23', 'F24'];

let deviceKeys = [];      // F-key labels present on the device, e.g. ['F13','F15']
let deviceMatrix = null;  // { rows, cols, cells: [{row,col,pos,occupied,fkey}] }
let mappings = {};        // { 'F13': {path, name}, ... }
let supportedKeys = null; // Set of labels usable as global hotkeys on this OS (null = unknown)
let modalKey = null;      // F-key currently shown in the edit modal
const iconCache = {};     // path -> data URL | null

// ── Helpers ───────────────────────────────────────────────────────────────
function setStatus(msg, kind) {
  const el = document.getElementById('status');
  if (!msg) { el.style.display = 'none'; return; }
  el.textContent = msg;
  el.className = 'status ' + (kind || 'ok');
  el.style.display = '';
}

function baseName(p) {
  const seg = p.split(/[\\/]/).pop() || p;
  return seg.replace(/\.(app|exe|lnk|sh|command)$/i, '');
}

function isSupported(fkey) { return !supportedKeys || supportedKeys.has(fkey); }

// Open a URL in the system browser (sidebar Home/Docs links).
function openExternal(url) {
  invoke('open_external', { url }).catch(e => setStatus(String(e), 'err'));
}
window.openExternal = openExternal;

// ── Sidebar view switching (Small Deck launcher ⇄ embedded Home / Docs) ─────
const VIEW_URLS = {
  home: 'https://xtiaconfiger.com',
  docs: 'https://xtiaconfiger.com/wiki.html#sd-overview',
};
const VIEW_FRAMES = { home: 'homeview-frame', docs: 'docview-frame' };
window.showView = function (view) {
  ['deck', 'home', 'docs'].forEach(function (v) {
    const el = document.getElementById('view-' + v);
    if (el) el.style.display = v === view ? 'flex' : 'none';
    const nav = document.getElementById('nav-' + v);
    if (nav) nav.classList.toggle('active', v === view);
  });
  // Load embedded pages lazily on first open (app startup never fetches remote
  // content); keep them loaded afterwards so reopening is instant.
  if (VIEW_URLS[view]) {
    const frame = document.getElementById(VIEW_FRAMES[view]);
    if (frame && (!frame.src || frame.src === 'about:blank')) frame.src = VIEW_URLS[view];
  }
};

// ── Settings modal (opened from the sidebar footer) ─────────────────────────
window.openSettings = function () {
  document.getElementById('settings-overlay').style.display = 'flex';
};
window.closeSettings = function () {
  document.getElementById('settings-overlay').style.display = 'none';
};
window.settingsBgClick = function (e) {
  if (e.target.id === 'settings-overlay') window.closeSettings();
};

window.quitApp = function () {
  invoke('quit_app').catch(() => {});
};

// Push the (localized) tray menu labels to the Rust side — the tray is built
// before the UI knows the language, so we relabel on load and on switch.
window.syncTrayLabels = function () {
  invoke('set_tray_labels', { show: t('tray_show'), quit: t('tray_quit') }).catch(() => {});
};

async function getIcon(path) {
  if (path in iconCache) return iconCache[path];
  let url = null;
  try { url = await invoke('app_icon', { path }); } catch (e) { url = null; }
  iconCache[path] = url;
  return url;
}

// A visual for a binding: app icon for programs, a glyph for folders/URLs.
function mediaSlot(entry, cls) {
  const slot = document.createElement('span');
  slot.className = 'icon-slot' + (cls ? ' ' + cls : '');
  if (!entry || !entry.path) return slot;
  if (entry.kind === 'folder')  { slot.textContent = '📁'; slot.classList.add('glyph'); return slot; }
  if (entry.kind === 'url')     { slot.textContent = '🌐'; slot.classList.add('glyph'); return slot; }
  if (entry.kind === 'command') { slot.textContent = '▶'; slot.classList.add('glyph'); return slot; }
  // app/file → try to extract the real icon
  getIcon(entry.path).then(url => {
    if (url) {
      const img = document.createElement('img');
      img.className = 'app-icon';
      img.src = url;
      slot.appendChild(img);
    }
  });
  return slot;
}

// ── Data load ───────────────────────────────────────────────────────────────
async function loadMappings() {
  try { mappings = await invoke('get_mappings'); } catch (e) { mappings = {}; }
}

async function readDevice(showErrors) {
  setStatus(t('reading'), 'ok');
  try {
    deviceMatrix = await invoke('read_device_matrix');
    deviceKeys = deviceMatrix.cells.filter(c => c.fkey).map(c => c.fkey);
    setStatus(t('read_ok', { n: deviceKeys.length }), 'ok');
  } catch (e) {
    deviceMatrix = null;
    deviceKeys = [];
    if (showErrors !== false) setStatus(String(e), 'err');
    else setStatus('', '');
  }
  render();
}
window.readDevice = readDevice;

// ── Render ───────────────────────────────────────────────────────────────────
let deckMode = 'matrix'; // 'matrix' | 'list'
window.setDeckMode = function (mode) {
  deckMode = mode;
  render();
};

window.showAllKeys = function () {
  document.getElementById('chk-showall').checked = true;
  render();
};

function render() {
  renderMatrix();
  renderList();
  updateDeckChrome();
}
window.render = render; // i18n.js calls this on language switch

// Empty state, view toggle, and the device status pill — the chrome around the
// matrix/list that depends on whether a device has been read.
function updateDeckChrome() {
  const hasDevice = !!(deviceMatrix && deviceMatrix.rows && deviceMatrix.cols);
  const keys = visibleKeys();
  const empty = !hasDevice && keys.length === 0;

  // Drop a stale selection only when the inspector is currently closed.
  // If the inspector is already open, the selection was made deliberately by the
  // user and should not be silently dropped by background render cycles (e.g. a
  // boot-time readDevice that completes after the user has already started
  // interacting with the UI).
  const inspectorOpen = document.getElementById('inspector').style.display !== 'none';
  if (!inspectorOpen && modalKey && !keys.includes(modalKey) && !deviceKeys.includes(modalKey)) {
    modalKey = null;
  }

  document.getElementById('empty-state').style.display = empty ? 'flex' : 'none';
  document.getElementById('deck-toolbar').style.display = empty ? 'none' : 'flex';
  document.getElementById('deck-body').style.display = empty ? 'none' : '';

  // Matrix needs a device read; without one, fall back to (and lock) list mode.
  const segMatrix = document.getElementById('seg-matrix');
  segMatrix.disabled = !hasDevice;
  segMatrix.classList.toggle('active', hasDevice && deckMode === 'matrix');
  document.getElementById('seg-list').classList.toggle('active', !hasDevice || deckMode === 'list');
  const showMatrix = hasDevice && deckMode === 'matrix';
  document.getElementById('matrix-wrap').style.display = showMatrix ? '' : 'none';
  document.getElementById('list-wrap').style.display = showMatrix ? 'none' : '';

  // Device status pill.
  const pill = document.getElementById('dev-pill');
  const text = document.getElementById('dev-pill-text');
  if (hasDevice) {
    pill.classList.add('on');
    text.textContent = deviceKeys.length
      ? t('dev_ready', { n: deviceKeys.length })
      : t('dev_nokeys');
  } else {
    pill.classList.remove('on');
    text.textContent = t('dev_none');
  }
}

function renderMatrix() {
  const host = document.getElementById('matrix');
  host.innerHTML = '';
  if (!deviceMatrix || !deviceMatrix.rows || !deviceMatrix.cols) return;
  // Fixed-size cells (not 1fr) so a small matrix doesn't stretch each key to
  // fill the row — keeps the compact, generator-style grid.
  host.style.gridTemplateColumns = `repeat(${deviceMatrix.cols}, 64px)`;

  deviceMatrix.cells.forEach(cell => {
    const el = document.createElement('div');
    el.className = 'mcell';
    el.title = cell.pos;

    if (!cell.occupied) {
      el.classList.add('empty');
    } else if (!cell.fkey) {
      el.classList.add('other'); // a normal key, not a Small Deck trigger
    } else {
      const ok = isSupported(cell.fkey);
      const m = mappings[cell.fkey];
      el.classList.add('fkey');
      if (!ok) el.classList.add('unsupported');
      if (cell.fkey === modalKey) el.classList.add('selected');

      const badge = document.createElement('div');
      badge.className = 'mcell-key';
      badge.textContent = cell.fkey;
      el.appendChild(badge);

      if (m && m.path) {
        el.appendChild(mediaSlot(m, 'big'));
      } else if (ok) {
        const plus = document.createElement('div');
        plus.className = 'mcell-plus';
        plus.textContent = '+';
        el.appendChild(plus);
      }
      if (ok) el.onclick = () => openModal(cell.fkey);
    }
    host.appendChild(el);
  });
}

function visibleKeys() {
  const showAll = document.getElementById('chk-showall').checked;
  const set = new Set(showAll ? ALL_FKEYS : deviceKeys);
  Object.keys(mappings).forEach(k => set.add(k)); // always surface bound keys
  return ALL_FKEYS.filter(k => set.has(k));
}

function renderList() {
  const host = document.getElementById('rows');
  const keys = visibleKeys();
  host.innerHTML = '';

  keys.forEach(key => {
    const m = mappings[key];
    const ok = isSupported(key);
    const row = document.createElement('div');
    row.className = 'row' + (ok ? '' : ' row-disabled') + (key === modalKey ? ' selected' : '');
    // Whole row selects the key and opens the docked inspector (Stream-Deck style).
    if (ok) { row.onclick = () => openModal(key); }

    const keyEl = document.createElement('div');
    keyEl.className = 'key';
    keyEl.textContent = key;

    const prog = document.createElement('div');
    prog.className = 'prog';
    if (!ok) {
      const warn = document.createElement('div');
      warn.className = 'unset';
      warn.textContent = t('unsupported');
      prog.appendChild(warn);
    } else if (m && m.path) {
      const head = document.createElement('div');
      head.className = 'prog-head';
      head.appendChild(mediaSlot(m));
      const name = document.createElement('span');
      name.className = 'name';
      name.textContent = m.name || baseName(m.path);
      head.appendChild(name);
      prog.appendChild(head);
      const path = document.createElement('div');
      path.className = 'path';
      path.textContent = m.path;
      prog.appendChild(path);
    } else {
      const unset = document.createElement('div');
      unset.className = 'unset';
      unset.textContent = t('unset');
      prog.appendChild(unset);
    }

    // A chevron hints the row opens the inspector (configure / test / clear there).
    const go = document.createElement('div');
    go.className = 'row-go';
    if (ok) go.textContent = '›';

    row.appendChild(keyEl);
    row.appendChild(prog);
    row.appendChild(go);
    host.appendChild(row);
  });
}

// ── Docked inspector (configure the selected key in place) ──────────────────
function openModal(fkey) {
  // Idempotent: if the same key is already selected, just make sure the
  // inspector is visible (avoid a full re-render that could race with
  // background async work).
  if (modalKey === fkey && document.getElementById('inspector').style.display === 'flex') {
    return;
  }
  modalKey = fkey;
  document.getElementById('modal-title').textContent = fkey;
  render();              // re-render to highlight the selected cell/row
  refreshModalBody();
  document.getElementById('inspector').style.display = 'flex';
}
function closeModal() {
  modalKey = null;
  document.getElementById('inspector').style.display = 'none';
  render();              // clear the selection highlight
}
function refreshModalBody() {
  const body = document.getElementById('modal-body');
  const m = modalKey ? mappings[modalKey] : null;
  body.innerHTML = '';
  if (m && m.path) {
    const head = document.createElement('div');
    head.className = 'prog-head';
    head.appendChild(mediaSlot(m, 'big'));
    const name = document.createElement('span');
    name.className = 'name';
    name.textContent = m.name || baseName(m.path);
    head.appendChild(name);
    body.appendChild(head);
    const path = document.createElement('div');
    path.className = 'path';
    path.textContent = m.path;
    body.appendChild(path);
  } else {
    const unset = document.createElement('div');
    unset.className = 'unset';
    unset.textContent = t('unset');
    body.appendChild(unset);
  }
  const has = !!(m && m.path);
  document.getElementById('m-test').disabled = !has;
  document.getElementById('m-clear').disabled = !has;
  // collapse the URL / command inputs whenever we refresh
  document.getElementById('modal-url').style.display = 'none';
  document.getElementById('url-input').value = (m && m.kind === 'url') ? m.path : '';
  document.getElementById('modal-cmd').style.display = 'none';
  document.getElementById('cmd-input').value = (m && m.kind === 'command') ? m.path : '';
}

// ── Actions ───────────────────────────────────────────────────────────────
async function afterChange() {
  await loadMappings();
  render();
  if (modalKey) refreshModalBody();
}

function urlName(url) {
  try { return new URL(url).hostname || url; } catch (e) { return url; }
}

async function saveMapping(key, path, name, kind) {
  try {
    await invoke('set_mapping', { key, path, name, kind });
    await afterChange();
  } catch (e) { setStatus(String(e), 'err'); }
}

// kind: 'app' (file picker) or 'folder' (directory picker)
async function pickFile(key, kind) {
  let selected;
  try { selected = await openFileDialog(t('pick_title'), kind === 'folder'); }
  catch (e) { setStatus(String(e), 'err'); return; }
  if (!selected) return;
  const path = Array.isArray(selected) ? selected[0] : selected;
  await saveMapping(key, path, baseName(path), kind);
}

async function saveUrl(key, url) {
  url = (url || '').trim();
  if (!url) return;
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) url = 'https://' + url; // default scheme
  await saveMapping(key, url, urlName(url), 'url');
}

async function saveCommand(key, cmd) {
  cmd = (cmd || '').trim();
  if (!cmd) return;
  const name = cmd.length > 24 ? cmd.slice(0, 24) + '…' : cmd;
  await saveMapping(key, cmd, name, 'command');
}

async function testKey(key) {
  try { await invoke('launch_key', { key }); setStatus(t('launch_ok'), 'ok'); }
  catch (e) { setStatus(String(e), 'err'); }
}

async function clearKey(key) {
  try { await invoke('remove_mapping', { key }); await afterChange(); }
  catch (e) { setStatus(String(e), 'err'); }
}

async function toggleAutostart() {
  const enabled = document.getElementById('chk-autostart').checked;
  try { await invoke('set_autostart', { enabled }); }
  catch (e) { setStatus(String(e), 'err'); }
}

async function initAutostart() {
  try {
    const on = await invoke('get_autostart');
    document.getElementById('chk-autostart').checked = !!on;
  } catch (e) { /* ignore */ }
}

// ── Window controls (custom title bar on Windows / Linux) ────────────────────
window.minimizeWindow = function () {
  invoke('plugin:window|minimize').catch(() => {});
};
window.toggleMaximize = function () {
  invoke('plugin:window|toggle_maximize').catch(() => {});
};
window.closeWindow = function () {
  // Mirror the existing close-to-tray behaviour: hide the window instead of
  // quitting, so the global hotkeys keep working in the background.
  invoke('plugin:window|hide').catch(() => {});
};

// ── Boot ──────────────────────────────────────────────────────────────────
window.addEventListener('DOMContentLoaded', async () => {
  // CSP-safe event wiring: inline on* handlers are blocked in the bundled build
  // (Tauri injects a script nonce, which makes 'unsafe-inline' ignored), so all
  // clicks are delegated via [data-act] / [data-arg] instead.
  document.addEventListener('click', e => {
    const el = e.target.closest('[data-act]');
    if (!el) return;
    const fn = window[el.getAttribute('data-act')];
    if (typeof fn === 'function') fn(el.getAttribute('data-arg'));
  });
  document.getElementById('chk-showall').addEventListener('change', render);
  document.getElementById('chk-autostart').addEventListener('change', toggleAutostart);
  document.getElementById('settings-overlay').addEventListener('click', settingsBgClick);

  // Show one inline input (url or command) at a time; hide the other.
  function toggleBox(showId, hideId, inputId) {
    if (!modalKey) return; // inspector closed; ignore
    const show = document.getElementById(showId);
    const hide = document.getElementById(hideId);
    const input = document.getElementById(inputId);
    if (!show || !hide || !input) return;
    hide.style.display = 'none';
    show.style.display = show.style.display === 'none' ? 'flex' : 'none';
    if (show.style.display === 'flex') input.focus();
  }
  document.getElementById('t-program').onclick = () => { if (modalKey) pickFile(modalKey, 'app'); };
  document.getElementById('t-folder').onclick  = () => { if (modalKey) pickFile(modalKey, 'folder'); };
  document.getElementById('t-url').onclick     = () => toggleBox('modal-url', 'modal-cmd', 'url-input');
  document.getElementById('t-command').onclick = () => toggleBox('modal-cmd', 'modal-url', 'cmd-input');
  document.getElementById('url-save').onclick = () => { if (modalKey) saveUrl(modalKey, document.getElementById('url-input').value); };
  document.getElementById('url-input').addEventListener('keydown', e => {
    if (e.key === 'Enter' && modalKey) saveUrl(modalKey, e.target.value);
  });
  document.getElementById('cmd-save').onclick = () => { if (modalKey) saveCommand(modalKey, document.getElementById('cmd-input').value); };
  document.getElementById('cmd-input').addEventListener('keydown', e => {
    // multi-line: plain Enter inserts a newline; Cmd/Ctrl+Enter saves
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && modalKey) saveCommand(modalKey, e.target.value);
  });
  document.getElementById('m-test').onclick = () => { if (modalKey) testKey(modalKey); };
  document.getElementById('m-clear').onclick = () => { if (modalKey) clearKey(modalKey); };
  document.getElementById('m-done').onclick = closeModal;

  // Esc closes whichever overlay is open (settings takes precedence).
  document.addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    if (document.getElementById('settings-overlay').style.display !== 'none') closeSettings();
    else if (document.getElementById('inspector').style.display !== 'none') closeModal();
  });

  try { supportedKeys = new Set(await invoke('supported_keys')); } catch (e) { supportedKeys = null; }
  await loadMappings();
  await initAutostart();
  render();
  readDevice(false); // try on startup; stay quiet if not plugged in
});
