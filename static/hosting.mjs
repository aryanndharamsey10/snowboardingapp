export function routePath(location) {
  if (location.hash.startsWith('#/')) return location.hash.slice(1);
  // Keep existing invitations served directly by Rust working.
  return /^\/p\/[a-f0-9]{32}(\/edit)?$/.test(location.pathname) ? location.pathname : '/';
}

export function routeUrl(base, path) {
  const url = new URL(base);
  url.hash = path;
  return url.href;
}

export function apiUrl(base, backend, path) {
  return new URL(path, backend || base.origin).href;
}
