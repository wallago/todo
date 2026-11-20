import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./index.css";
import { registerServiceWorker } from "./register-sw";
import { Store } from "./store";

const root = document.getElementById("root");
if (!root) {
  throw new Error("index.html has no #root element");
}

const store = new Store(window.localStorage);

createRoot(root).render(
  <StrictMode>
    <App store={store} />
  </StrictMode>,
);

registerServiceWorker();
