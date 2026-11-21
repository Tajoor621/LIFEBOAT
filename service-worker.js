/* LIFEBOAT static-shell worker. No private data or API caching. */
"use strict";
const CACHE_PREFIX = "lifeboat-shell-";
const CACHE_NAME = CACHE_PREFIX + "v1.1.0";
const BASE = new URL("/LIFEBOAT/", self.location.origin);
const SHELL = [
  new URL("index.html", BASE).href,
  new URL("offline.html", BASE).href,
  new URL("pwa.css", BASE).href,
  new URL("pwa.js", BASE).href,
  new URL("manifest.webmanifest", BASE).href,
  new URL("assets/branding/lifeboat-mark.svg", BASE).href,
  new URL("assets/branding/lifeboat-pwa-icon.svg", BASE).href,
  new URL("assets/branding/lifeboat-icon-192.png", BASE).href,
  new URL("assets/branding/lifeboat-icon-512.png", BASE).href,
  new URL("assets/branding/lifeboat-favicon-32.png", BASE).href,
  new URL("assets/branding/lifeboat-apple-touch-icon-180.png", BASE).href
];
self.addEventListener("install", event => {
  event.waitUntil(caches.open(CACHE_NAME).then(cache => cache.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener("activate", event => {
  event.waitUntil(caches.keys().then(keys => Promise.all(
    keys.filter(key => key.startsWith(CACHE_PREFIX) && key !== CACHE_NAME).map(key => caches.delete(key))
  )).then(() => self.clients.claim()));
});
self.addEventListener("fetch", event => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin) return;
  if (!url.pathname.startsWith("/LIFEBOAT/")) return;
  if (SHELL.includes(url.href)) {
    event.respondWith(caches.match(request).then(cached => cached || fetch(request)));
    return;
  }
  if (request.mode === "navigate") {
    event.respondWith(fetch(request).catch(async () => (await caches.match(new URL("offline.html", BASE).href)) || Response.error()));
  }
});
