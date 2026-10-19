// Browser boundary only: DOM, URL state and calls into the Rust/WASM engine.
// Matching, ranking, facets, pagination and highlight ranges are owned by
// scripture-search; this file only places the ranges it returns into the page.
const here = new URL(import.meta.url);
const version = here.search;
const assetsBase = new URL('./', here).href;

// Wrap the [start, end) Unicode-scalar ranges computed in Rust in <mark>, using
// text nodes only (no HTML is ever built from result text).
function marked(text, ranges) {
  const out = document.createDocumentFragment();
  const chars = Array.from(text);
  let at = 0;
  for (const [start, end] of ranges || []) {
    if (start < at || end > chars.length || start >= end) continue;
    if (start > at) out.append(chars.slice(at, start).join(''));
    const mark = document.createElement('mark');
    mark.textContent = chars.slice(start, end).join('');
    out.append(mark);
    at = end;
  }
  if (at < chars.length) out.append(chars.slice(at).join(''));
  return out;
}

async function loadEngine(binding, wasm) {
  const engine = await import(
    binding ? 'data:text/javascript;base64,' + binding : assetsBase + 'discovery.js' + version
  );
  await engine.default({
    module_or_path: wasm
      ? Uint8Array.from(atob(wasm), (c) => c.charCodeAt(0))
      : assetsBase + 'discovery_bg.wasm' + version,
  });
  return engine;
}

const root = document.getElementById('discovery');
if (root) {
  const base = root.dataset.base;
  const linkOf = (path) => {
    if (!root.dataset.previewRoot || !path.startsWith(base)) return path;
    const [name, anchor] = path.split('#');
    const [file, search] = name.split('?');
    return root.dataset.previewRoot + file.slice(base.length) +
      (file.endsWith('/') ? 'index.html' : '') + (search ? '?' + search : '') +
      (anchor ? '#' + anchor : '');
  };
  const form = document.getElementById('discover-form');
  const input = document.getElementById('discover-query');
  const count = document.getElementById('result-count');
  const results = document.getElementById('search-results');
  const previous = document.getElementById('previous-results');
  const next = document.getElementById('next-results');
  let documents = [], engine, current = { offset: 0, next: null }, timer;
  const fields = ['q', 'kind', 'testament', 'book', 'artist', 'topic', 'sort', 'mode'];
  const checks = ['red_letter', 'with_art', 'fuzzy', 'exact_spelling'];
  const defaults = { sort: 'relevance', mode: 'all' };
  const control = (key) => form.elements.namedItem(key);
  const restore = () => {
    const params = new URLSearchParams(location.search);
    for (const key of fields) {
      control(key).value = (params.get(key) ||
        (key === 'kind' ? root.dataset.defaultKind : defaults[key] || '')).slice(0, 256);
    }
    // Links made before the match modes used phrase=1.
    if (params.get('phrase') === '1' && !params.get('mode')) control('mode').value = 'phrase';
    for (const key of checks) control(key).checked = params.get(key) === '1';
    const offset = Number(params.get('offset'));
    current.offset = Number.isSafeInteger(offset) && offset >= 0 ? offset : 0;
  };
  const query = () => {
    const value = { offset: current.offset };
    for (const key of fields) value[key] = control(key).value;
    for (const key of checks) value[key] = control(key).checked;
    return value;
  };
  const element = (tag, className, text) => {
    const node = document.createElement(tag);
    node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  // A verse link carries the words marked in the results (King James spellings and
  // near spellings included) so the chapter can mark them too.
  const withHighlight = (href, q, terms) => {
    if (!q.q.trim()) return href;
    const [path, anchor] = href.split('#');
    const phrase = q.mode === 'phrase';
    const params = new URLSearchParams({
      hl: phrase ? q.q : (terms.length ? terms.join(' ') : q.q),
    });
    if (phrase) params.set('phrase', '1');
    return path + '?' + params.toString() + (anchor ? '#' + anchor : '');
  };
  const jumpBox = document.getElementById('jump');
  const notes = document.getElementById('search-notes');
  const counts = document.getElementById('book-counts');
  const labelOf = (key, q) => {
    if (checks.includes(key)) return control(key).parentElement.textContent.trim();
    if (key === 'mode') return q.mode === 'any' ? 'Any of the words' : 'The exact phrase';
    const node = control(key);
    if (node instanceof HTMLSelectElement) return node.selectedOptions[0]?.textContent || q[key];
    return q[key];
  };
  const render = (push = false, submitted = false) => {
    if (!engine) return;
    const q = query();
    const found = JSON.parse(engine.search(JSON.stringify(q)));
    current = found;
    if (submitted && found.jump && found.jump.passage) {
      location.href = linkOf(found.jump.href);
      return;
    }
    const params = new URLSearchParams();
    for (const key of fields) if (q[key] && q[key] !== defaults[key]) params.set(key, q[key]);
    for (const key of checks) if (q[key]) params.set(key, '1');
    if (found.offset) params.set('offset', String(found.offset));
    const url = location.pathname + (params.size ? '?' + params.toString() : '');
    if (!root.dataset.previewRoot) {
      if (push) history.pushState(null, '', url);
      else history.replaceState(null, '', url);
    }
    count.textContent = `${found.total.toLocaleString()} ${
      found.total === 1 ? 'result' : 'results'
    }${q.q.trim() ? ` for “${q.q.trim()}”` : ''}`;
    jumpBox.replaceChildren();
    if (found.jump) {
      const box = element('div', 'jump');
      const label = element('p', '');
      label.append(found.jump.passage ? 'Open ' : 'Open the book of ');
      label.append(element('strong', '', found.jump.label));
      const go = element('a', 'discover-button', 'Read');
      go.href = linkOf(found.jump.href);
      box.append(label, go);
      jumpBox.append(box);
    }
    notes.replaceChildren();
    if (found.variants.length) {
      notes.append(
        element(
          'p',
          'search-note',
          `Also searched in the King James spelling: ${found.variants.join(', ')}.`,
        ),
      );
    }
    if (found.near.length) {
      notes.append(
        element(
          'p',
          'search-note',
          `Near spellings searched, and their results marked: ${found.near.join(', ')}.`,
        ),
      );
    }
    counts.replaceChildren();
    if (q.q.trim() && found.books.length > 1) {
      for (const entry of found.books) {
        const chip = element('button', 'count-chip');
        chip.type = 'button';
        chip.setAttribute('aria-pressed', String(q.book === entry.book));
        chip.append(entry.book, ' ', element('span', 'c', entry.count.toLocaleString()));
        chip.addEventListener('click', () => {
          control('book').value = q.book === entry.book ? '' : entry.book;
          current.offset = 0;
          render(true);
        });
        counts.append(chip);
      }
    }
    results.replaceChildren();
    found.ids.forEach((id, index) => {
      const item = documents[id], marks = found.marks[index] || { title: [], text: [] };
      const card = element('article', `search-card ${item.kind}`);
      const href = linkOf(
        item.kind === 'verse' ? withHighlight(item.href, q, found.terms) : item.href,
      );
      if (item.image) {
        const link = element('a', 'result-image');
        link.href = href;
        link.tabIndex = -1;
        link.setAttribute('aria-hidden', 'true');
        const image = document.createElement('img');
        image.src = linkOf(item.image);
        image.alt = '';
        image.loading = 'lazy';
        image.decoding = 'async';
        link.append(image);
        card.append(link);
      }
      const body = element('div', 'result-body');
      body.append(
        element(
          'p',
          'eyebrow',
          item.kind === 'art' ? 'ARTWORK' : `${item.testament.toUpperCase()} · KING JAMES VERSION`,
        ),
      );
      const heading = element('h2', ''), link = element('a', '');
      link.href = href;
      link.append(marked(item.title, marks.title));
      heading.append(link);
      if (found.fuzzy[index]) heading.append(element('span', 'near-badge', 'near spelling'));
      body.append(heading);
      const text = element('p', item.kind === 'verse' ? 'result-verse' : 'meta');
      text.append(marked(item.text, marks.text));
      body.append(text);
      body.append(
        element(
          'p',
          'result-context',
          item.kind === 'verse'
            ? 'Read the full chapter →'
            : 'View artwork, passage & attribution →',
        ),
      );
      card.append(body);
      results.append(card);
    });
    document.getElementById('empty-results').hidden = found.total !== 0;
    previous.disabled = found.offset === 0;
    next.disabled = found.next === null;
    document.getElementById('page-label').textContent = found.total
      ? `${found.offset + 1}–${found.offset + found.ids.length} of ${found.total.toLocaleString()}`
      : '';
    const active = document.getElementById('active-filters');
    active.replaceChildren();
    for (const key of [...fields.filter((key) => !['q', 'sort'].includes(key)), ...checks]) {
      if (!q[key] || q[key] === defaults[key]) continue;
      const label = labelOf(key, q);
      const chip = element('button', 'active-chip', `${label} ×`);
      chip.type = 'button';
      chip.setAttribute('aria-label', `Remove ${label} filter`);
      chip.addEventListener('click', () => {
        if (checks.includes(key)) control(key).checked = false;
        else control(key).value = defaults[key] || '';
        current.offset = 0;
        render(true);
      });
      active.append(chip);
    }
  };
  const clear = () => {
    for (const key of fields) control(key).value = defaults[key] || '';
    for (const key of checks) control(key).checked = false;
    current.offset = 0;
    render(true);
    input.focus();
  };
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    current.offset = 0;
    render(true, true);
  });
  form.addEventListener('change', () => {
    current.offset = 0;
    render(true);
  });
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      current.offset = 0;
      render();
    }, 140);
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && input.value) {
      event.preventDefault();
      input.value = '';
      current.offset = 0;
      render(true);
    }
    if (event.key === 'ArrowDown') {
      const first = results.querySelector('h2 a');
      if (first) {
        event.preventDefault();
        first.focus();
      }
    }
  });
  document.getElementById('clear-filters').addEventListener('click', clear);
  document.getElementById('empty-clear').addEventListener('click', clear);
  previous.addEventListener('click', () => {
    current.offset = Math.max(0, current.offset - 40);
    render(true);
    results.scrollIntoView({ block: 'start' });
  });
  next.addEventListener('click', () => {
    if (current.next !== null) {
      current.offset = current.next;
      render(true);
      results.scrollIntoView({ block: 'start' });
    }
  });
  document.querySelectorAll('[data-topic]').forEach((link) =>
    link.addEventListener('click', (event) => {
      event.preventDefault();
      control('topic').value = link.dataset.topic;
      control('kind').value = 'verse';
      control('artist').value = '';
      current.offset = 0;
      render(true);
    })
  );
  globalThis.addEventListener('popstate', () => {
    restore();
    render();
  });
  restore();
  try {
    engine = await loadEngine(
      document.getElementById('inline-binding')?.textContent,
      document.getElementById('inline-wasm')?.textContent,
    );
    // A reference typed into any page's search box opens its passage at once.
    const typed = JSON.parse(engine.passage(input.value));
    if (typed && !root.dataset.previewRoot) {
      location.replace(
        `${base}${typed.slug}/${typed.chapter}/` + (typed.verse ? `#v${typed.verse}` : ''),
      );
    } else {
      let text = document.getElementById('inline-catalogue')?.textContent;
      if (!text) {
        const response = await fetch(base + 'discovery/catalogue.json' + version, {
          credentials: 'omit',
        });
        if (!response.ok) throw new Error('catalogue unavailable');
        text = await response.text();
      }
      documents = JSON.parse(text);
      engine.initialize(text);
      render();
    }
  } catch (_) {
    count.textContent = 'Search could not load. You can still browse the books and artwork.';
    previous.disabled = true;
    next.disabled = true;
    const fallback = element('a', 'discover-button', 'Browse books');
    fallback.href = base;
    results.append(fallback);
  }
} else {
  // A chapter opened from a search: mark the searched words in its verses.
  const params = new URLSearchParams(location.search);
  const words = (params.get('hl') || '').slice(0, 256);
  const text = document.querySelector('.text');
  if (words.trim() && text) {
    try {
      const engine = await loadEngine();
      const phrase = params.get('phrase') === '1';
      const skip = (node) => node.parentElement?.closest('.vn, .xc, .xr, figure, .sr');
      const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => (skip(node) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
      });
      const nodes = [];
      while (walker.nextNode()) nodes.push(walker.currentNode);
      let hits = 0, first = null;
      for (const node of nodes) {
        const ranges = JSON.parse(engine.mark(node.data, words, phrase));
        if (!ranges.length) continue;
        hits += ranges.length;
        first ||= node.parentElement.closest('.v');
        node.replaceWith(marked(node.data, ranges));
      }
      const note = document.createElement('p');
      note.className = 'hl-note';
      note.setAttribute('role', 'status');
      note.append(
        hits
          ? `${hits} ${
            hits === 1 ? 'match' : 'matches'
          } for “${words.trim()}” marked in this chapter.`
          : `No match for “${words.trim()}” in this chapter.`,
      );
      const back = document.createElement('a');
      back.href = assetsBase.replace(/discovery\/$/, '') + 'search/?' +
        new URLSearchParams({ q: words }).toString();
      back.textContent = 'Back to the results';
      const clear = document.createElement('a');
      clear.href = location.pathname + location.hash;
      clear.textContent = 'Clear highlights';
      note.append(back, clear);
      text.before(note);
      if (!location.hash && first) first.scrollIntoView({ block: 'center' });
    } catch (_) {
      // Highlighting is an enhancement; the chapter reads the same without it.
    }
  }
}
