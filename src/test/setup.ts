import { afterEach, beforeEach } from "vitest";
import { cleanup } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

import { installMatchMedia } from "./match-media";

// jsdom does not implement matchMedia. Provide a safe default (no media matched)
// before each test; tests that need control re-install it themselves.
beforeEach(() => {
  installMatchMedia(false);
});

afterEach(() => {
  cleanup();
});
