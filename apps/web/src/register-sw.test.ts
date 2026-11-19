import { expect, test, vi } from "vitest";
import { registerServiceWorker } from "./register-sw";

test("registers the worker at the site root", () => {
  const register = vi.fn<ServiceWorkerContainer["register"]>();
  registerServiceWorker({ serviceWorker: { register } }, true);
  expect(register).toHaveBeenCalledWith(`${import.meta.env.BASE_URL}sw.js`);
});

test("skips a browser without service workers", () => {
  expect(registerServiceWorker({}, true)).toBeUndefined();
});

test("skips a dev build", () => {
  const register = vi.fn<ServiceWorkerContainer["register"]>();
  registerServiceWorker({ serviceWorker: { register } }, false);
  expect(register).not.toHaveBeenCalled();
});
