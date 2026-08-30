// Global test setup for Vitest
// This file runs before each test file

// Suppress console noise during tests
// import "@testing-library/jest-dom";

// Auto-unmount React renders between tests so testing-library queries
// don't see leftover DOM from earlier tests in the same file.
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});
