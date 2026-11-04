const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const script = fs.readFileSync(path.join(__dirname, '../tracker.js'), 'utf8');

function collector({ storageDenied = false, hostname = 'example.com', webdriver = false } = {}) {
  const calls = [];
  const values = new Map();
  const storage = {
    getItem(key) { if (storageDenied) throw new Error('denied'); return values.get(key) || null; },
    setItem(key, value) { if (storageDenied) throw new Error('denied'); values.set(key, value); },
  };
  const location = { hostname, protocol: 'https:', pathname: '/docs' };
  const history = {
    pushState(_state, _title, target) { location.pathname = new URL(target, 'https://example.com').pathname; },
    replaceState(_state, _title, target) { location.pathname = new URL(target, 'https://example.com').pathname; },
  };
  const window = {};
  window.parent = window;
  vm.runInNewContext(script, {
    document: {
      currentScript: { dataset: { site: 'site-1', endpoint: 'https://api.example.com/record', app: 'app-1' } },
      referrer: 'https://search.example/results?private=value#fragment',
    },
    location, history, window, navigator: { webdriver, userAgent: 'Example browser' },
    localStorage: storage, sessionStorage: storage, URL,
    fetch(endpoint, options) { calls.push({ endpoint, headers: { ...options.headers }, body: JSON.parse(options.body) }); return Promise.resolve(); },
    setTimeout(callback) { callback(); return 1; }, clearTimeout() {}, addEventListener() {},
  });
  return { calls, history };
}

test('collects page paths and referring origins, then tracks SPA changes', () => {
  const { calls, history } = collector();
  assert.deepEqual(calls[0], {
    endpoint: 'https://api.example.com/record',
    headers: { 'Content-Type': 'application/json', 'X-App': 'app-1' },
    body: { site: 'site-1', page: '/docs', referrer: 'https://search.example' },
  });
  history.pushState(null, '', '/about?secret=value#fragment');
  history.replaceState(null, '', '/pricing?secret=value');
  assert.deepEqual(calls.slice(1).map(call => call.body), [
    { site: 'site-1', page: '/about' }, { site: 'site-1', page: '/pricing' },
  ]);
});

test('storage restrictions do not prevent collection', () => {
  const { calls } = collector({ storageDenied: true });
  assert.equal(calls.length, 1);
});

test('local development and declared automation do not record', () => {
  assert.equal(collector({ hostname: 'localhost' }).calls.length, 0);
  assert.equal(collector({ webdriver: true }).calls.length, 0);
});
