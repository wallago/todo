import { expect, test } from "vitest";
import raw from "../public/manifest.webmanifest?raw";

/** The manifest fields these tests read. */
interface Manifest {
  icons: { src: string; sizes: string; purpose?: string }[];
}

const manifest: Manifest = JSON.parse(raw);
const shipped = Object.keys(import.meta.glob("../public/icons/*"));

test("has what browsers need to offer an install", () => {
  expect(manifest).toMatchObject({
    name: "Todo",
    start_url: "./",
    display: "standalone",
  });
  const sizes = manifest.icons.map((icon) => icon.sizes);
  expect(sizes).toContain("192x192");
  expect(sizes).toContain("512x512");
  expect(manifest.icons.some((icon) => icon.purpose === "maskable")).toBe(true);
});

test("lists only icons that ship in public/", () => {
  for (const icon of manifest.icons) {
    expect(shipped).toContain(`../public/${icon.src}`);
  }
});
