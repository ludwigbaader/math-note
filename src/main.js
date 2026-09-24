'use strict';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const SESSION_KEY = 'mathnote.session';
const SAVE_DELAY_MS = 400;

const els = {};
for (const id of ['note-list', 'new-note', 'help-btn', 'help', 'tabbar', 'workspace', 'empty',
  'title', 'mirror', 'input', 'status-vars', 'status-save']) {
  els[id.replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = document.getElementById(id);
}

// notes: NoteMeta[] from the database. tabs: open notes with their live content.
const state = { notes: [], tabs: [], activeId: null };

// ---------------------------------------------------------------- helpers

const activeTab = () => state.tabs.find((t) => t.id === state.activeId) || null;
const findTab = (id) => state.tabs.find((t) => t.id === id) || null;

function titleFor(title, body) {
  const t = (title || '').trim();
  if (t) return t;
  const line = (body || '').split('\n').find((l) => l.trim());
  return line ? line.trim().slice(0, 40) : 'Untitled';
}

function escapeHtml(s) {
  return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
}

function formatDate(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const now = new Date();
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }
  const opts = { day: 'numeric', month: 'short' };
  if (d.getFullYear() !== now.getFullYear()) opts.year = 'numeric';
  return d.toLocaleDateString(undefined, opts);
}

function loadSession() {
  try {
    const s = JSON.parse(localStorage.getItem(SESSION_KEY) || '{}');
    return { open: Array.isArray(s.open) ? s.open : [], active: s.active ?? null };
  } catch {
    return { open: [], active: null };
  }
}

function saveSession() {
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify({
      open: state.tabs.map((t) => t.id),
      active: state.activeId,
    }));
  } catch { /* storage unavailable: nothing to do */ }
}

// ---------------------------------------------------------------- rendering

function renderTabs() {
  els.tabbar.replaceChildren(...state.tabs.map((tab) => {
    const el = document.createElement('div');
    el.className = 'tab' + (tab.id === state.activeId ? ' active' : '') + (tab.dirty ? ' dirty' : '');
    el.setAttribute('role', 'tab');
    const label = document.createElement('span');
    label.className = 'tab-label';
    label.textContent = titleFor(tab.title, tab.content);
    const close = document.createElement('button');
    close.className = 'tab-close';
    close.title = 'Close tab (⌘W)';
    close.textContent = '×';
    close.addEventListener('click', (e) => { e.stopPropagation(); closeTab(tab.id); });
    el.append(label, close);
    el.addEventListener('click', () => activate(tab.id));
    el.addEventListener('auxclick', (e) => { if (e.button === 1) closeTab(tab.id); });
    return el;
  }));
}

function renderSidebar() {
  const sorted = [...state.notes].sort((a, b) =>
    b.updated_at < a.updated_at ? -1 : b.updated_at > a.updated_at ? 1 : b.id - a.id);
  els.noteList.replaceChildren(...sorted.map((note) => {
    const tab = findTab(note.id);
    const li = document.createElement('li');
    li.className = 'note' + (note.id === state.activeId ? ' active' : '') + (tab ? ' open' : '');
    const title = document.createElement('div');
    title.className = 'note-title';
    title.textContent = tab ? titleFor(tab.title, tab.content) : titleFor(note.title, note.preview);
    const meta = document.createElement('div');
    meta.className = 'note-meta';
    meta.textContent = formatDate(note.updated_at);
    const del = document.createElement('button');
    del.className = 'note-delete';
    del.title = 'Delete note';
    del.textContent = '×';
    del.addEventListener('click', (e) => {
      e.stopPropagation();
      if (del.dataset.armed) {
        deleteNote(note.id);
        return;
      }
      del.dataset.armed = '1';
      del.textContent = 'Delete?';
      setTimeout(() => { delete del.dataset.armed; del.textContent = '×'; }, 2500);
    });
    li.append(title, meta, del);
    li.addEventListener('click', () => openNote(note.id));
    return li;
  }));
}

// Draws the note text plus the grey results into the layer behind the textarea.
function renderMirror(content, results) {
  const byLine = new Map(results.map((r) => [r.line, r]));
  const lines = content.split('\n');
  const parts = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    parts.push(escapeHtml(line));
    const r = byLine.get(i);
    if (r) {
      const gap = /\s$/.test(line) ? '' : ' ';
      parts.push(`<span class="res ${r.kind}">${gap}${escapeHtml(r.text)}</span>`);
    }
    parts.push('\n');
  }
  parts.push(' '); // keeps the trailing empty line as tall as in the textarea
  els.mirror.innerHTML = parts.join('');
  syncScroll();
}

function renderVariables(variables) {
  if (!variables.length) {
    els.statusVars.textContent = '';
    return;
  }
  els.statusVars.replaceChildren(...variables.flatMap((v, i) => {
    const b = document.createElement('b');
    b.textContent = v.name;
    const nodes = [b, document.createTextNode(` = ${v.value}`)];
    if (i < variables.length - 1) {
      const sep = document.createElement('span');
      sep.className = 'sep';
      sep.textContent = '·';
      nodes.push(sep);
    }
    return nodes;
  }));
}

function setSaveStatus(text) {
  els.statusSave.textContent = text;
}

function syncScroll() {
  els.mirror.scrollTop = els.input.scrollTop;
  els.mirror.scrollLeft = els.input.scrollLeft;
}

function showEmpty() {
  state.activeId = null;
  els.workspace.hidden = true;
  els.empty.hidden = false;
  renderTabs();
  renderSidebar();
  saveSession();
}

// ---------------------------------------------------------------- evaluation

let evalInFlight = false;
let evalPending = false;

async function requestEvaluate() {
  if (evalInFlight) {
    evalPending = true;
    return;
  }
  const tab = activeTab();
  if (!tab) return;
  evalInFlight = true;
  const { id, content } = tab;
  try {
    const out = await invoke('evaluate', { content });
    tab.results = out.results;
    tab.variables = out.variables;
    if (state.activeId === id && !evalPending) {
      renderMirror(content, out.results);
      renderVariables(out.variables);
    }
  } catch (err) {
    console.error('evaluate failed', err);
  } finally {
    evalInFlight = false;
    if (evalPending) {
      evalPending = false;
      requestEvaluate();
    }
  }
}

// ---------------------------------------------------------------- persistence

async function refreshNotes() {
  state.notes = await invoke('list_notes');
  renderSidebar();
}

function updateNoteMeta(meta) {
  const i = state.notes.findIndex((n) => n.id === meta.id);
  if (i >= 0) state.notes[i] = meta;
  else state.notes.push(meta);
}

function scheduleSave(tab) {
  tab.dirty = true;
  clearTimeout(tab.saveTimer);
  tab.saveTimer = setTimeout(() => saveNow(tab), SAVE_DELAY_MS);
  setSaveStatus('Edited');
}

// Saves are chained per tab so that two quick edits can never land out of order.
function saveNow(tab) {
  clearTimeout(tab.saveTimer);
  tab.saveChain = (tab.saveChain || Promise.resolve()).then(async () => {
    if (!tab.dirty) return;
    const snapshot = { title: tab.title, content: tab.content };
    if (tab.id === state.activeId) setSaveStatus('Saving…');
    try {
      const meta = await invoke('save_note', { id: tab.id, ...snapshot });
      if (tab.title === snapshot.title && tab.content === snapshot.content) tab.dirty = false;
      updateNoteMeta(meta);
      renderSidebar();
      renderTabs();
      if (tab.id === state.activeId) setSaveStatus(tab.dirty ? 'Edited' : 'Saved');
    } catch (err) {
      console.error('save failed', err);
      if (tab.id === state.activeId) setSaveStatus('Save failed');
    }
  });
  return tab.saveChain;
}

async function flushAll() {
  await Promise.all(state.tabs.filter((t) => t.dirty).map((t) => saveNow(t)));
}

// ---------------------------------------------------------------- tabs & notes

function activate(id) {
  const tab = findTab(id);
  if (!tab) return;
  const previous = activeTab();
  if (previous && previous !== tab) {
    previous.scrollTop = els.input.scrollTop;
    previous.selection = [els.input.selectionStart, els.input.selectionEnd];
  }

  state.activeId = id;
  els.workspace.hidden = false;
  els.empty.hidden = true;
  els.title.value = tab.title;
  // Focus first: WebKit's focus() reveals the caret asynchronously and would undo the
  // scroll position set below (assigning `value` also moves the caret to the end).
  els.input.focus();
  els.input.value = tab.content;
  const [start, end] = tab.selection || [0, 0];
  els.input.setSelectionRange(start, end);
  els.input.scrollTop = tab.scrollTop || 0;
  renderMirror(tab.content, tab.results || []);
  renderVariables(tab.variables || []);
  setSaveStatus(tab.dirty ? 'Edited' : 'Saved');
  renderTabs();
  renderSidebar();
  saveSession();
  requestEvaluate();
}

async function openNote(id, focus = true) {
  if (findTab(id)) {
    if (focus) activate(id);
    return;
  }
  const note = await invoke('get_note', { id });
  state.tabs.push({
    id: note.id,
    title: note.title,
    content: note.content,
    dirty: false,
    results: [],
    variables: [],
    scrollTop: 0,
    selection: [0, 0],
  });
  renderTabs();
  if (focus) activate(id);
  else saveSession();
}

async function newNote() {
  const note = await invoke('create_note');
  updateNoteMeta({ id: note.id, title: note.title, preview: '', updated_at: note.updated_at });
  state.tabs.push({
    id: note.id, title: '', content: '', dirty: false, results: [], variables: [], scrollTop: 0, selection: [0, 0],
  });
  activate(note.id);
}

async function closeTab(id) {
  const tab = findTab(id);
  if (!tab) return;
  if (tab.dirty) await saveNow(tab);
  const index = state.tabs.indexOf(tab);
  state.tabs.splice(index, 1);
  if (state.activeId === id) {
    const next = state.tabs[index] || state.tabs[index - 1];
    if (next) activate(next.id);
    else showEmpty();
  } else {
    renderTabs();
    renderSidebar();
    saveSession();
  }
}

async function deleteNote(id) {
  const tab = findTab(id);
  if (tab) {
    clearTimeout(tab.saveTimer);
    tab.dirty = false;
    state.tabs.splice(state.tabs.indexOf(tab), 1);
  }
  try {
    await invoke('delete_note', { id });
  } catch (err) {
    console.error('delete failed', err);
  }
  state.notes = state.notes.filter((n) => n.id !== id);
  if (state.activeId === id) {
    const next = state.tabs[0];
    if (next) activate(next.id);
    else showEmpty();
  } else {
    renderTabs();
    renderSidebar();
    saveSession();
  }
}

function cycleTab(step) {
  if (state.tabs.length < 2) return;
  const i = state.tabs.findIndex((t) => t.id === state.activeId);
  const next = state.tabs[(i + step + state.tabs.length) % state.tabs.length];
  activate(next.id);
}

// ---------------------------------------------------------------- events

function bindEvents() {
  els.newNote.addEventListener('click', () => newNote());
  els.helpBtn.addEventListener('click', () => {
    els.help.hidden = !els.help.hidden;
    els.helpBtn.classList.toggle('on', !els.help.hidden);
  });

  els.input.addEventListener('input', () => {
    const tab = activeTab();
    if (!tab) return;
    tab.content = els.input.value;
    renderMirror(tab.content, tab.results); // instant feedback, results refresh right after
    scheduleSave(tab);
    requestEvaluate();
  });
  els.input.addEventListener('scroll', syncScroll);
  els.input.addEventListener('keydown', (e) => {
    if (e.key === 'Tab' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      document.execCommand('insertText', false, '\t');
    }
  });

  els.title.addEventListener('input', () => {
    const tab = activeTab();
    if (!tab) return;
    tab.title = els.title.value;
    scheduleSave(tab);
    renderTabs();
    renderSidebar();
  });
  els.title.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      els.input.focus();
    }
  });

  document.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (e.ctrlKey && e.key === 'Tab') {
      e.preventDefault();
      cycleTab(e.shiftKey ? -1 : 1);
    } else if (mod && e.shiftKey && (e.code === 'BracketRight' || e.code === 'BracketLeft')) {
      e.preventDefault();
      cycleTab(e.code === 'BracketRight' ? 1 : -1);
    }
  });

  // Native menu items (⌘N, ⌘W, ⌘S) arrive as events from Rust.
  listen('menu', (event) => {
    switch (event.payload) {
      case 'new-note': newNote(); break;
      case 'close-tab': if (state.activeId != null) closeTab(state.activeId); break;
      case 'save': { const tab = activeTab(); if (tab) saveNow(tab); break; }
      default: break;
    }
  });

  // The window close button and ⌘Q are intercepted in Rust so nothing is lost.
  listen('app-close-requested', async () => {
    try {
      await flushAll();
    } finally {
      invoke('finish_close');
    }
  });

  window.addEventListener('blur', () => flushAll());
}

// ---------------------------------------------------------------- start

async function init() {
  bindEvents();
  await refreshNotes();
  const session = loadSession();
  for (const id of session.open) {
    if (state.notes.some((n) => n.id === id)) {
      try { await openNote(id, false); } catch (err) { console.error('could not reopen note', id, err); }
    }
  }
  if (session.active != null && findTab(session.active)) activate(session.active);
  else if (state.tabs.length) activate(state.tabs[0].id);
  else if (state.notes.length) await openNote(state.notes[0].id); // first launch: most recent note
  else showEmpty();
}

init().catch((err) => console.error('init failed', err));
