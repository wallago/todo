// The service worker: precaches the build on install, so the app opens
// offline, and drops the previous build's cache once it takes over.

// Swapped by `vite-sw.ts` at build time for every built file, relative to the
// site root, and for a hash of that list.
declare const __PRECACHE__: string[];
declare const __VERSION__: string;
// This file's global scope: a service worker, not the plain worker the
// WebWorker lib types `self` as. Declaring it needs the file to be a module.
declare const self: ServiceWorkerGlobalScope;

export {};

/** Prefix of every cache this worker owns, so it never drops another's. */
const PREFIX = "precache-";
/** This build's cache. */
const CACHE = `${PREFIX}${__VERSION__}`;

self.addEventListener("install", (event) => {
  // One file that fails to cache fails the install, so a worker never takes
  // over with half an app.
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(__PRECACHE__)));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith(PREFIX) && key !== CACHE)
            .map((key) => caches.delete(key)),
        ),
      ),
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) {
    return;
  }
  if (request.mode === "navigate") {
    // Pages: the network first, so a deploy shows on reload; the cached shell
    // when offline. The site is one page, so every path gets the shell.
    event.respondWith(fetch(request).catch(() => fromCache("./")));
    return;
  }
  // Built files are hashed and never change: the cache first.
  event.respondWith(caches.match(request).then((hit) => hit ?? fetch(request)));
});

/** `url` from this build's cache, or a network error if it isn't there. */
async function fromCache(url: string): Promise<Response> {
  return (await caches.match(url)) ?? Response.error();
}
