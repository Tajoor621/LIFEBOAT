/* LIFEBOAT static web shell. Never cache private records or API responses. */
"use strict";
const CACHE_PREFIX = "lifeboat-shell-";
const CACHE_NAME = CACHE_PREFIX + "v1.2.0";
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
  new URL("assets/branding/lifeboat-icon-maskable-512.png", BASE).href,
  new URL("assets/branding/lifeboat-shortcut-96.png", BASE).href,
  new URL("assets/branding/lifeboat-favicon-32.png", BASE).href,
  new URL("assets/branding/lifeboat-apple-touch-icon-180.png", BASE).href,
  new URL("screenshots/lifeboat-mobile.png", BASE).href,
  new URL("screenshots/lifeboat-desktop.png", BASE).href
];
self.addEventListener("install", event => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE_NAME);
    await cache.addAll(SHELL);
    await self.skipWaiting();
  })());
});
self.addEventListener("activate", event => {
  event.waitUntil((async () => {
    const keys = await caches.keys();
    await Promise.all(keys.filter(key => key.startsWith(CACHE_PREFIX) && key !== CACHE_NAME).map(key => caches.delete(key)));
    await self.clients.claim();
  })());
});
self.addEventListener("fetch", event => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin || !url.pathname.startsWith("/LIFEBOAT/")) return;
  if (SHELL.includes(url.href)) {
    event.respondWith((async () => {
      const cached = await caches.match(request);
      if (cached) return cached;
      try { return await fetch(request); }
      catch { return Response.error(); }
    })());
    return;
  }
  if (request.mode === "navigate") {
    event.respondWith((async () => {
      try { return await fetch(request); }
      catch { return (await caches.match(new URL("offline.html", BASE).href)) || Response.error(); }
    })());
  }
});
