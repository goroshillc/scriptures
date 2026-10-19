// The reader's browser boundary. Every page reads, links and expands without it.
//
// What lives here is only what a browser alone can do: keys, the clipboard, the zoom
// view, the gallery filter, the reader's place, and the private record's storage (this
// browser's localStorage and one first-party cookie, which no server ever reads). Every
// rule and number of the record (what counts, how much, levels, streaks, checks and
// file validation) is computed by the Rust engine compiled to WebAssembly; this file
// only passes it events and places what it returns into the page with textContent.
const here = new URL(import.meta.url);
const version = here.search;
const assets = new URL('./', here).href;
const base = assets.replace(/discovery\/$/, '');
const STORE = 'scriptures.record.v1';
const COOKIE = '__Host-scriptures';
const PLACE = 'scriptures.place';
const WJ = 'scriptures.wj-only';
const ROPE = 'scriptures.rope';

let enginePromise;
function engine() {
  enginePromise ||= import(assets + 'discovery.js' + version).then(async (mod) => {
    await mod.default({ module_or_path: assets + 'discovery_bg.wasm' + version });
    return mod;
  });
  return enginePromise;
}

const read = (key) => {
  try {
    return localStorage.getItem(key);
  } catch (_) {
    return null;
  }
};
const store = (key, value) => {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
    return true;
  } catch (_) {
    return false;
  }
};
const pad = (n) => String(n).padStart(2, '0');
const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
};
const typing = (target) => {
  const tag = target && target.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' ||
    (target && target.isContentEditable);
};
const el = (tag, className, text) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

// ---------------------------------------------------------------------------------------
// The private record: localStorage for the record, one host-only cookie for its essentials.
// ---------------------------------------------------------------------------------------

function readCookie() {
  const found = document.cookie.split('; ').find((c) => c.startsWith(COOKIE + '='));
  return found ? decodeURIComponent(found.slice(COOKIE.length + 1)) : null;
}

// The essentials of the profile in use, so a record survives cleared site storage:
// version, id, name, the four experience totals and completed readings.
function writeCookie(raw) {
  try {
    const record = JSON.parse(raw);
    const p = record.profiles.find((x) => x.id === record.active);
    if (!p) return;
    const xp = p.xp || {};
    const value = [
      'v1',
      p.id,
      p.name,
      xp.reading || 0,
      xp.prayer || 0,
      xp.memory || 0,
      xp.study || 0,
      p.readings || 0,
      record.record ? 1 : 0,
    ].map((v) => String(v))
      .map(encodeURIComponent).join('|');
    document.cookie = `${COOKIE}=${encodeURIComponent(value)}; Max-Age=31536000; Path=/; ` +
      'Secure; SameSite=Strict';
  } catch (_) {
    // A cookie is a convenience; the record itself is in localStorage.
  }
}

function clearCookie() {
  document.cookie = `${COOKIE}=; Max-Age=0; Path=/; Secure; SameSite=Strict`;
}

// Rebuilds a record from the cookie's essentials when site storage was cleared.
async function fromCookie() {
  const raw = readCookie();
  if (!raw) return null;
  const parts = raw.split('|').map(decodeURIComponent);
  if (parts.length !== 9 || parts[0] !== 'v1') return null;
  const [, id, name, reading, prayer, memory, study, readings, record] = parts;
  const num = (v) => (/^\d{1,16}$/.test(v) ? Number(v) : 0);
  const file = JSON.stringify({
    format: 'goroshi-scriptures-profile-1',
    profile: {
      id,
      name,
      xp: { reading: num(reading), prayer: num(prayer), memory: num(memory), study: num(study) },
      readings: num(readings),
    },
  });
  try {
    const e = await engine();
    let s = e.progress_new('restore', 'Reader');
    s = e.progress_import(s, file);
    s = e.progress_change(s, JSON.stringify({ kind: 'remove', id: 'restore' }));
    if (record === '0') s = e.progress_change(s, JSON.stringify({ kind: 'record', on: false }));
    return s;
  } catch (_) {
    return null;
  }
}

async function loadRecord() {
  const raw = read(STORE);
  if (raw) {
    try {
      return (await engine()).progress_check(raw);
    } catch (_) {
      return null;
    }
  }
  const restored = await fromCookie();
  if (restored) saveRecord(restored);
  return restored;
}

function saveRecord(raw) {
  store(STORE, raw);
  writeCookie(raw);
}

function hasRecord() {
  return read(STORE) !== null || readCookie() !== null;
}

let noteTimer;
function edgeNote(text) {
  let note = document.querySelector('.edge-note');
  if (!note) {
    note = el('p', 'edge-note');
    note.setAttribute('role', 'status');
    document.body.append(note);
  }
  note.textContent = text;
  note.classList.add('on');
  clearTimeout(noteTimer);
  noteTimer = setTimeout(() => note.classList.remove('on'), 3600);
}

// Applies one event to the record. Returns the engine's credit, or null.
async function credit(event, seed = 0) {
  const raw = await loadRecord();
  if (!raw) return null;
  const e = await engine();
  try {
    const result = JSON.parse(e.progress_apply(raw, JSON.stringify(event), today(), seed));
    saveRecord(JSON.stringify(result.store));
    if (result.credit.xp > 0 && result.store.notes) edgeNote(result.credit.text);
    return result.credit;
  } catch (_) {
    return null;
  }
}

// ---------------------------------------------------------------------------------------
// Every page: keys, the header search's passage jump.
// ---------------------------------------------------------------------------------------

document.addEventListener('keydown', (event) => {
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
  if (typing(event.target) || document.querySelector('dialog[open]')) return;
  const rel = event.key === 'ArrowLeft' ? 'prev' : event.key === 'ArrowRight' ? 'next' : null;
  if (!rel) return;
  const link = document.querySelector(`.pager a[rel="${rel}"]`);
  if (link) {
    event.preventDefault();
    location.href = link.href;
  }
});

const headerForm = document.querySelector('form.hsearch');
if (headerForm) {
  headerForm.addEventListener('submit', async (event) => {
    const q = headerForm.elements.namedItem('q').value.trim();
    if (!q) return;
    event.preventDefault();
    try {
      const found = JSON.parse((await engine()).passage(q));
      if (found) {
        location.href = `${base}${found.slug}/${found.chapter}/` +
          (found.verse ? `#v${found.verse}` : '');
        return;
      }
    } catch (_) {
      // Fall through to the search page.
    }
    headerForm.submit();
  });
}

// ---------------------------------------------------------------------------------------
// Chapter pages: verse tools, words of Christ only, the reader's place, reading credit.
// ---------------------------------------------------------------------------------------

const article = document.querySelector('article[data-book]');

function verseText(summary) {
  const copy = summary.cloneNode(true);
  copy.querySelectorAll('.vn, .xc, .sr').forEach((n) => n.remove());
  return copy.textContent.replace(/\s+/g, ' ').trim();
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    const area = el('textarea');
    area.value = text;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.append(area);
    area.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch (_) {
      ok = false;
    }
    area.remove();
    return ok;
  }
}

if (article) {
  const book = article.dataset.book;
  const chapter = Number(article.dataset.chapter);
  const heading = article.querySelector('h1');
  store(
    PLACE,
    JSON.stringify({ href: location.pathname, label: heading ? heading.textContent : '' }),
  );

  const wj = document.getElementById('wj-only');
  if (wj) {
    wj.checked = read(WJ) === '1';
    wj.addEventListener('change', () => store(WJ, wj.checked ? '1' : null));
  }

  article.addEventListener('toggle', (event) => {
    const details = event.target;
    if (!(details instanceof HTMLDetailsElement) || !details.open) return;
    const tools = details.querySelector('.vt');
    if (!tools || tools.dataset.ready) return;
    tools.dataset.ready = '1';
    const anchor = tools.querySelector('a[data-ref]');
    const reference = anchor ? anchor.dataset.ref : '';
    const summary = details.querySelector('summary');
    const copy = el('button', 'link-button', 'Copy the verse');
    copy.type = 'button';
    copy.addEventListener('click', async () => {
      const ok = await copyText(`${verseText(summary)}\n${reference} (KJV)`);
      copy.textContent = ok ? 'Copied' : 'Copy unavailable';
      setTimeout(() => (copy.textContent = 'Copy the verse'), 1800);
    });
    tools.append(copy);
    if (hasRecord()) {
      const keep = el('button', 'link-button', 'Keep for memory');
      keep.type = 'button';
      keep.addEventListener('click', async () => {
        const href = location.pathname + anchor.getAttribute('href');
        const result = await credit({
          kind: 'keep',
          reference,
          href,
          text: verseText(summary),
        });
        keep.textContent = result ? result.text.replace(/\.$/, '') : 'Kept';
        keep.disabled = true;
      });
      tools.append(keep);
    }
  }, true);

  if (hasRecord()) {
    engine().catch(() => {});
    let visible = 0;
    let since = document.visibilityState === 'visible' ? performance.now() : null;
    let reachedEnd = false;
    let done = false;
    const elapsed = () => visible + (since === null ? 0 : performance.now() - since);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') since = performance.now();
      else if (since !== null) {
        visible += performance.now() - since;
        since = null;
      }
    });
    const attempt = async () => {
      if (done || !reachedEnd) return;
      const result = await credit({
        kind: 'chapter',
        book,
        chapter,
        verses: Number(article.dataset.verses),
        words: Number(article.dataset.words),
        visible_ms: Math.floor(elapsed()),
        reached_end: true,
      });
      if (result && (result.xp > 0 || /Already/.test(result.text))) done = true;
    };
    const end = document.getElementById('chapter-end');
    if (end && 'IntersectionObserver' in globalThis) {
      new IntersectionObserver((entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          reachedEnd = true;
          attempt();
        }
      }).observe(end);
    }
    const timer = setInterval(() => {
      if (done) clearInterval(timer);
      else attempt();
    }, 15000);
    article.addEventListener('click', async (event) => {
      const link = event.target.closest && event.target.closest('a[data-xref]');
      if (!link || event.ctrlKey || event.metaKey || event.shiftKey || event.button !== 0) return;
      event.preventDefault();
      const target = link.getAttribute('href').replace(base, '');
      await Promise.race([
        credit({ kind: 'xref', target }),
        new Promise((resolve) => setTimeout(resolve, 600)),
      ]);
      location.href = link.href;
    });
  }
}

// ---------------------------------------------------------------------------------------
// Home: continue reading.
// ---------------------------------------------------------------------------------------

const cont = document.getElementById('continue');
if (cont) {
  try {
    const place = JSON.parse(read(PLACE) || 'null');
    if (place && typeof place.href === 'string' && place.href.startsWith(base) && place.label) {
      cont.append('Continue reading at ');
      const link = el('a', '', place.label);
      link.href = place.href;
      cont.append(link);
      cont.hidden = false;
    }
  } catch (_) {
    // Nothing remembered.
  }
}

// ---------------------------------------------------------------------------------------
// Plates: the zoom view, and time spent with a plate.
// ---------------------------------------------------------------------------------------

document.querySelectorAll('a.zoom').forEach((link) => {
  link.addEventListener('click', (event) => {
    if (event.ctrlKey || event.metaKey || event.shiftKey || event.button !== 0) return;
    event.preventDefault();
    const dialog = el('dialog', 'zoomer');
    const bar = el('div', 'zbar');
    const label = el(
      'span',
      '',
      `Full scan, ${link.dataset.width} by ${link.dataset.height} pixels`,
    );
    const actions = el('span');
    const fit = el('button', '', 'Actual size');
    fit.type = 'button';
    const close = el('button', '', 'Close');
    close.type = 'button';
    actions.append(fit, ' ', close);
    bar.append(label, actions);
    const stage = el('div', 'zstage');
    const picture = document.createElement('picture');
    for (
      const [type, src] of [['image/avif', link.dataset.avif], ['image/webp', link.dataset.webp]]
    ) {
      if (!src) continue; // the desktop edition's zoom image is JPEG only
      const source = document.createElement('source');
      source.type = type;
      source.srcset = src;
      picture.append(source);
    }
    const img = document.createElement('img');
    img.src = link.href;
    img.width = Number(link.dataset.width);
    img.height = Number(link.dataset.height);
    img.alt = link.querySelector('img') ? link.querySelector('img').alt : '';
    img.decoding = 'async';
    picture.append(img);
    stage.append(picture);
    dialog.append(bar, stage);
    const toggle = () => {
      dialog.classList.toggle('actual');
      fit.textContent = dialog.classList.contains('actual') ? 'Fit to screen' : 'Actual size';
    };
    fit.addEventListener('click', toggle);
    img.addEventListener('click', toggle);
    close.addEventListener('click', () => dialog.close());
    dialog.addEventListener('close', () => dialog.remove());
    document.body.append(dialog);
    dialog.showModal();
    close.focus();
  });
});

const plate = document.querySelector('main figure[data-plate] a.zoom');
if (plate && hasRecord() && 'IntersectionObserver' in globalThis) {
  const slug = plate.closest('figure').dataset.plate;
  let shown = 0;
  let since = null;
  let sent = false;
  new IntersectionObserver((entries) => {
    const on = entries.some((e) => e.intersectionRatio >= 0.5);
    if (on && since === null) since = performance.now();
    if (!on && since !== null) {
      shown += performance.now() - since;
      since = null;
    }
  }, { threshold: [0, 0.5, 1] }).observe(plate);
  const check = setInterval(async () => {
    const total = shown +
      (since === null || document.visibilityState !== 'visible' ? 0 : performance.now() - since);
    if (!sent && total >= 8000) {
      sent = true;
      clearInterval(check);
      await credit({ kind: 'plate', slug, visible_ms: Math.floor(total) });
    }
  }, 2000);
}

// ---------------------------------------------------------------------------------------
// The gallery's book filter (the whole gallery reads in order without it).
// ---------------------------------------------------------------------------------------

const filter = document.getElementById('gallery-filter');
if (filter) {
  const select = document.getElementById('gallery-book');
  const count = document.getElementById('gallery-count');
  const sections = [...document.querySelectorAll('section.gallery-book')];
  const apply = (push) => {
    const book = select.value;
    let shown = 0;
    for (const section of sections) {
      section.hidden = Boolean(book) && section.dataset.book !== book;
      if (!section.hidden) shown += section.querySelectorAll('li').length;
    }
    count.textContent = `${shown} plate${shown === 1 ? '' : 's'}`;
    const url = location.pathname + (book ? `?book=${encodeURIComponent(book)}` : '');
    if (push) history.pushState(null, '', url);
  };
  filter.addEventListener('submit', (event) => event.preventDefault());
  select.addEventListener('change', () => apply(true));
  globalThis.addEventListener('popstate', () => {
    select.value = new URLSearchParams(location.search).get('book') || '';
    apply(false);
  });
  const wanted = new URLSearchParams(location.search).get('book') || '';
  if ([...select.options].some((o) => o.value === wanted)) select.value = wanted;
  apply(false);
}

// ---------------------------------------------------------------------------------------
// The prayer page.
// ---------------------------------------------------------------------------------------

const prayerPage = document.getElementById('prayer');
if (prayerPage) {
  const status = document.getElementById('prayer-status');
  const counter = document.getElementById('counter');
  const countEl = document.getElementById('count');
  let rope = { date: today(), count: 0, kept: 0 };
  try {
    const saved = JSON.parse(read(ROPE) || 'null');
    if (saved && saved.date === today() && Number.isSafeInteger(saved.count)) rope = saved;
  } catch (_) {
    // A fresh count.
  }
  const show = () => {
    countEl.textContent = rope.count.toLocaleString();
    store(ROPE, JSON.stringify(rope));
  };
  counter.hidden = false;
  show();
  document.getElementById('knot').addEventListener('click', () => {
    rope.count = Math.min(rope.count + 1, 100000);
    show();
  });
  document.getElementById('count-reset').addEventListener('click', () => {
    rope = { date: today(), count: 0, kept: 0 };
    show();
  });
  const keepCount = document.getElementById('count-save');
  if (!hasRecord()) keepCount.hidden = true;
  keepCount.addEventListener('click', async () => {
    const fresh = rope.count - rope.kept;
    if (fresh <= 0) {
      status.textContent = 'Nothing new to keep.';
      return;
    }
    const result = await credit({ kind: 'repetitions', count: Math.min(fresh, 10000) });
    if (result) {
      rope.kept = rope.count;
      show();
      status.textContent = result.text;
    }
  });

  const renderOwn = async () => {
    const raw = await loadRecord();
    if (!raw) return;
    const view = JSON.parse((await engine()).progress_view(raw, today()));
    document.querySelectorAll('[data-pray]').forEach((button) => {
      button.hidden = !view.record;
      const prayed = view.prayed_today.includes(button.dataset.pray);
      button.disabled = prayed;
      if (prayed) button.textContent = 'Recorded today';
    });
    const section = document.getElementById('own-prayers');
    section.hidden = !view.record;
    const list = document.getElementById('own-list');
    list.replaceChildren();
    for (const name of view.prayers) {
      const item = el('li', '', name + ' ');
      const said = el(
        'button',
        'link-button',
        view.prayed_today.includes(name) ? 'Recorded today' : 'Prayed',
      );
      said.type = 'button';
      said.disabled = view.prayed_today.includes(name);
      said.addEventListener('click', async () => {
        const result = await credit({ kind: 'prayer', rule: name });
        if (result) status.textContent = result.text;
        renderOwn();
      });
      const remove = el('button', 'link-button', 'Remove');
      remove.type = 'button';
      remove.addEventListener('click', async () => {
        await credit({ kind: 'remove_prayer', name });
        renderOwn();
      });
      item.append(said, ' ', remove);
      list.append(item);
    }
  };
  document.querySelectorAll('[data-pray]').forEach((button) =>
    button.addEventListener('click', async () => {
      const result = await credit({ kind: 'prayer', rule: button.dataset.pray });
      if (result) status.textContent = result.text;
      renderOwn();
    })
  );
  document.getElementById('own-add').addEventListener('submit', async (event) => {
    event.preventDefault();
    const input = document.getElementById('own-name');
    const name = input.value.trim();
    if (!name) return;
    try {
      const raw = await loadRecord();
      const e = await engine();
      const result = JSON.parse(
        e.progress_apply(raw, JSON.stringify({ kind: 'add_prayer', name }), today(), 0),
      );
      saveRecord(JSON.stringify(result.store));
      status.textContent = result.credit.text;
      input.value = '';
    } catch (error) {
      status.textContent = String(error);
    }
    renderOwn();
  });
  if (hasRecord()) renderOwn();
}

// ---------------------------------------------------------------------------------------
// The profile page.
// ---------------------------------------------------------------------------------------

const profilePage = document.getElementById('profile');
if (profilePage) {
  const status = document.getElementById('p-status');
  const say = (text) => (status.textContent = text);
  const fileInput = document.getElementById('load-file');
  const words = [...document.querySelectorAll('#milestone-words li')];
  const bookNames = new Map(
    [...document.querySelectorAll('#book-names li')].map((li) => [li.dataset.slug, li]),
  );
  const dayLabel = (day) =>
    new Date(day * 86400000).toLocaleDateString(undefined, {
      timeZone: 'UTC',
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  const newId = () => {
    const bytes = new Uint8Array(6);
    crypto.getRandomValues(bytes);
    return 'r-' + [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('');
  };
  const milestoneWords = (level) => {
    const named = words.find((li) => li.dataset.level === String(level));
    if (named) return named;
    const rotation = words.filter((li) => li.dataset.level === '0');
    if (!rotation.length || level < 100) return null;
    return rotation[(Math.floor(level / 100) - 1) % rotation.length];
  };
  let confirmTimer;
  const twoStep = (button, label, action) => {
    button.addEventListener('click', async () => {
      if (button.dataset.armed) {
        delete button.dataset.armed;
        button.textContent = label;
        await action();
        return;
      }
      button.dataset.armed = '1';
      button.textContent = 'Press again to confirm';
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => {
        delete button.dataset.armed;
        button.textContent = label;
      }, 4000);
    });
  };

  const change = async (body) => {
    const raw = await loadRecord();
    try {
      saveRecord((await engine()).progress_change(raw, JSON.stringify(body)));
      return true;
    } catch (error) {
      say(String(error));
      return false;
    }
  };

  const render = async () => {
    const raw = await loadRecord();
    document.getElementById('profile-start').hidden = Boolean(raw);
    document.getElementById('profile-view').hidden = !raw;
    if (!raw) return;
    const e = await engine();
    const v = JSON.parse(e.progress_view(raw, today()));
    document.getElementById('p-initial').textContent = v.initial;
    document.getElementById('p-name').textContent = v.name;
    document.getElementById('ring-text').textContent = String(v.total_level);
    const mean = v.skills.reduce((sum, s) => sum + s.ppm, 0) / (v.skills.length * 1000000);
    document.getElementById('ring-fill').setAttribute(
      'stroke-dasharray',
      `${(mean * 282.743).toFixed(3)} 282.743`,
    );
    const totals = document.getElementById('p-totals');
    totals.replaceChildren();
    for (
      const [label, value] of [
        ['Total level', String(v.total_level)],
        ['Experience', v.total_xp],
        ['Days in a row', String(v.streak)],
        ['Longest', String(v.longest)],
      ]
    ) {
      const item = el('div', '', label);
      item.prepend(el('strong', '', value));
      totals.append(item);
    }
    const skills = document.getElementById('p-skills');
    skills.replaceChildren();
    for (const s of v.skills) {
      const li = el('li', 'skill');
      const line = el('div', 'bar-line');
      const fill = el('span');
      fill.style.width = s.percent;
      line.append(fill);
      line.setAttribute('role', 'img');
      line.setAttribute('aria-label', `${s.percent} of the way to level ${s.level + 1}`);
      li.append(
        el('span', 'name', s.label),
        line,
        el('span', 'lvl', `Level ${s.level.toLocaleString()}`),
        el(
          'span',
          'detail',
          `${s.xp} experience · ${s.to_next} to level ${(s.level + 1).toLocaleString()}`,
        ),
      );
      skills.append(li);
    }
    document.getElementById('p-readings').textContent =
      `The whole Bible read through: ${v.readings.toLocaleString()} time${
        v.readings === 1 ? '' : 's'
      }. ` +
      `This reading: ${v.chapters_read.toLocaleString()} of 1,189 chapters.`;
    const map = document.getElementById('p-books');
    map.replaceChildren();
    for (const b of v.books) {
      const li = el('li', b.complete ? 'done' : '');
      const link = el('a', 'n', b.name);
      const named = bookNames.get(b.slug);
      link.href = named ? named.dataset.href : base;
      const line = el('div', 'bar-line');
      const fill = el('span');
      fill.style.width = `${((b.read / b.chapters) * 100).toFixed(2)}%`;
      line.append(fill);
      li.append(link, `${b.read} of ${b.chapters}`, line);
      map.append(li);
    }
    const memory = document.getElementById('p-memory');
    memory.replaceChildren();
    if (!v.memory.length) memory.append(el('p', 'meta', 'No verses kept yet.'));
    for (const m of v.memory) {
      const box = el('div', 'milestone');
      const ref = el('a', '', m.reference);
      ref.href = m.href;
      const head = el('p');
      head.append(ref, m.passed ? ` · said back ${m.passed} time${m.passed === 1 ? '' : 's'}` : '');
      const sayBack = el('button', 'link-button', 'Say it back');
      sayBack.type = 'button';
      const release = el('button', 'link-button', 'Let go');
      release.type = 'button';
      const actions = el('p', 'form-row');
      actions.append(sayBack, release);
      box.append(head, actions);
      sayBack.addEventListener('click', () => {
        const seed = Math.floor(Math.random() * 1000);
        const hidden = JSON.parse(e.progress_blanks(m.text, seed));
        const form = el('form', 'said');
        const parts = m.text.split(/\s+/);
        const inputs = [];
        parts.forEach((word, i) => {
          if (hidden.includes(i)) {
            const input = el('input');
            input.type = 'text';
            input.size = Math.max(3, word.length);
            input.setAttribute('aria-label', `Word ${i + 1}`);
            input.autocomplete = 'off';
            inputs.push(input);
            form.append(input, ' ');
          } else {
            form.append(word + ' ');
          }
        });
        const check = el('button', 'btn quiet', 'Check');
        check.type = 'submit';
        form.append(check);
        form.addEventListener('submit', async (event) => {
          event.preventDefault();
          const result = await credit({
            kind: 'check',
            reference: m.reference,
            answers: inputs.map((i) => i.value),
          }, seed);
          say(result ? result.text : 'Unavailable.');
          if (result && result.xp > 0) render();
        });
        box.replaceChildren(head, form);
        if (inputs[0]) inputs[0].focus();
      });
      release.addEventListener('click', async () => {
        await credit({ kind: 'release', reference: m.reference });
        render();
      });
      memory.append(box);
    }
    const milestones = document.getElementById('p-milestones');
    milestones.replaceChildren();
    if (!v.milestones.length) milestones.append(el('p', 'meta', 'None yet.'));
    for (const m of v.milestones) {
      const box = el('div', 'milestone');
      const skill = m.skill.charAt(0).toUpperCase() + m.skill.slice(1);
      box.append(
        el(
          'p',
          'meta',
          `${skill} reached level ${m.level.toLocaleString()} on ${dayLabel(m.day)}.`,
        ),
      );
      const said = milestoneWords(m.level);
      if (said) {
        const quote = el('p', 'said wj', said.textContent);
        const cite = el('p', 'meta');
        const link = el('a', '', said.dataset.ref);
        link.href = said.dataset.href;
        cite.append(link);
        box.append(quote, cite);
      }
      milestones.append(box);
    }
    const notes = document.getElementById('p-notes');
    notes.replaceChildren();
    for (const n of v.notes) notes.append(el('li', '', `${dayLabel(n.day)} · ${n.text}`));
    const profiles = document.getElementById('p-profiles');
    profiles.replaceChildren();
    for (const [id, name, initial] of v.profiles) {
      const button = el('button');
      button.type = 'button';
      button.setAttribute('aria-pressed', String(id === v.active));
      button.append(el('span', 'initial', initial), name);
      button.addEventListener('click', async () => {
        if (await change({ kind: 'use', id })) render();
      });
      const li = el('li');
      li.append(button);
      profiles.append(li);
    }
    document.getElementById('set-record').checked = v.record;
    document.getElementById('set-notes').checked = v.notes_on;
  };

  document.getElementById('start-form').addEventListener('submit', async (event) => {
    event.preventDefault();
    const name = document.getElementById('start-name').value;
    try {
      saveRecord((await engine()).progress_new(newId(), name));
      say('Your record is kept in this browser.');
      render();
    } catch (error) {
      say(String(error));
    }
  });
  document.getElementById('add-form').addEventListener('submit', async (event) => {
    event.preventDefault();
    const input = document.getElementById('add-name');
    if (await change({ kind: 'add', id: newId(), name: input.value })) {
      input.value = '';
      render();
    }
  });
  document.getElementById('rename-form').addEventListener('submit', async (event) => {
    event.preventDefault();
    const input = document.getElementById('rename-name');
    const raw = await loadRecord();
    const id = JSON.parse(raw).active;
    if (await change({ kind: 'rename', id, name: input.value })) {
      input.value = '';
      render();
    }
  });
  document.getElementById('set-record').addEventListener('change', async (event) => {
    await change({ kind: 'record', on: event.target.checked });
    render();
  });
  document.getElementById('set-notes').addEventListener('change', async (event) => {
    await change({ kind: 'notes', on: event.target.checked });
    render();
  });
  twoStep(document.getElementById('remove-profile'), 'Remove this profile', async () => {
    const raw = await loadRecord();
    const id = JSON.parse(raw).active;
    if (await change({ kind: 'remove', id })) render();
  });
  twoStep(document.getElementById('clear-all'), 'Clear every record on this device', () => {
    store(STORE, null);
    clearCookie();
    say('Every record on this device was cleared.');
    render();
  });
  document.querySelectorAll('[data-download]').forEach((button) =>
    button.addEventListener('click', async () => {
      const raw = await loadRecord();
      if (!raw) return;
      const e = await engine();
      const id = JSON.parse(raw).active;
      const file = e.progress_export(raw, id);
      const blob = new Blob([file], { type: 'application/json' });
      const link = el('a');
      link.href = URL.createObjectURL(blob);
      link.download = `scriptures-progress-${today()}.json`;
      document.body.append(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(link.href), 4000);
      say('Your progress file was saved by your browser.');
    })
  );
  document.querySelectorAll('[data-load]').forEach((button) =>
    button.addEventListener('click', () => fileInput.click())
  );
  fileInput.addEventListener('change', async () => {
    const file = fileInput.files && fileInput.files[0];
    fileInput.value = '';
    if (!file) return;
    if (file.size > 4 * 1024 * 1024) {
      say('That file is too large to be a progress file.');
      return;
    }
    const text = await file.text();
    const e = await engine();
    try {
      let raw = await loadRecord();
      if (!raw) {
        raw = e.progress_new('loading', 'Reader');
        raw = e.progress_import(raw, text);
        raw = e.progress_change(raw, JSON.stringify({ kind: 'remove', id: 'loading' }));
      } else {
        raw = e.progress_import(raw, text);
      }
      saveRecord(raw);
      say('Your progress was loaded.');
      render();
    } catch (error) {
      say(`That file was not loaded: ${error}`);
    }
  });
  render().catch(() => say('Your record could not be shown in this browser.'));
}
