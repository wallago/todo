/** What registering needs of `navigator`: absent outside a secure context. */
export interface Navigator {
  serviceWorker?: Pick<ServiceWorkerContainer, "register">;
}

/**
 * Registers the worker the build writes at the site root, so the app can be
 * installed and opens offline. Production builds only: under `vite` dev there
 * is no worker, and an old one would keep serving a stale build.
 */
export function registerServiceWorker(
  nav: Navigator = navigator,
  production: boolean = import.meta.env.PROD,
): Promise<ServiceWorkerRegistration> | undefined {
  if (!production || nav.serviceWorker === undefined) {
    return undefined;
  }
  return nav.serviceWorker.register(`${import.meta.env.BASE_URL}sw.js`);
}
