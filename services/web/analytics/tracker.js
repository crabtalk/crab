(() => {
  const tag = document.currentScript;
  if (!tag) return;
  const site = tag.dataset.site;
  const endpoint = tag.dataset.endpoint;
  const app = tag.dataset.app;
  const host = location.hostname;
  const local = host === "localhost" || host === "[::1]" || /^127\./.test(host) || /\.(local|localhost)$/.test(host);
  function ignored() {
    try { return localStorage.getItem("crab_analytics_ignore") === "true"; } catch { return false; }
  }
  const robot = navigator.webdriver || /headless|phantomjs|selenium|puppeteer|playwright/i.test(navigator.userAgent);
  if (
    !site || !endpoint || !app ||
    robot ||
    location.protocol === "file:" ||
    (local && !("allowLocalhost" in tag.dataset)) ||
    window !== window.parent ||
    ignored()
  ) return;

  const last = "crab_analytics_view";
  // document.referrer is the page load's, so only its first view sends it.
  let referrer;
  try {
    const referring = new URL(document.referrer);
    if (["http:", "https:"].includes(referring.protocol)) referrer = referring.origin;
  } catch {}
  function view() {
    const page = location.pathname;
    const now = Date.now();
    try {
      const seen = JSON.parse(sessionStorage.getItem(last) || "null");
      if (seen && seen.site === site && seen.page === page && now - seen.at < 60000) return;
      sessionStorage.setItem(last, JSON.stringify({ site, page, at: now }));
    } catch {}
    fetch(endpoint, {
      method: "POST",
      keepalive: true,
      headers: { "Content-Type": "application/json", "X-App": app },
      body: JSON.stringify({ site, page, referrer }),
    }).catch(() => {});
    referrer = undefined;
  }
  let path = location.pathname;
  let timer;
  function moved() {
    if (location.pathname === path) return;
    path = location.pathname;
    clearTimeout(timer);
    timer = setTimeout(view, 100);
  }
  for (const method of ["pushState", "replaceState"]) {
    const original = history[method];
    history[method] = function (...args) {
      const result = original.apply(this, args);
      moved();
      return result;
    };
  }
  addEventListener("popstate", moved);
  view();
})();
