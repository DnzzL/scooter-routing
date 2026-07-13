const CACHE = 'scooter-route-v3';
const URLS = ['/', '/favicon.svg', '/icon.svg', '/manifest.json'];

self.addEventListener('install', e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(URLS)));
  self.skipWaiting();
});

self.addEventListener('activate', e => {
  e.waitUntil(caches.keys().then(ks => Promise.all(ks.filter(k => k !== CACHE).map(k => caches.delete(k)))));
  self.clients.claim();
});

self.addEventListener('fetch', e => {
  if (e.request.url.startsWith(self.location.origin) && !e.request.url.includes('/api/')) {
    // Network-first for HTML (always get fresh), cache-first for assets
    if (e.request.mode === 'navigate') {
      e.respondWith(
        fetch(e.request).then(res => {
          const clone = res.clone();
          caches.open(CACHE).then(c => c.put(e.request, clone));
          return res;
        }).catch(() => caches.match(e.request))
      );
    } else {
      e.respondWith(
        caches.match(e.request).then(r => r || fetch(e.request).then(res => {
          const clone = res.clone();
          caches.open(CACHE).then(c => c.put(e.request, clone));
          return res;
        }))
      );
    }
  }
});
