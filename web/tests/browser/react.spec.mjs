import { test, expect } from "@playwright/test";
test("real React StrictMode mount, update and unmount lifecycle", async ({ page }) => {
  const errors = []; page.on("pageerror", error => errors.push(error.message));
  await page.goto("/tests/browser/host.html");
  const labels = await page.evaluate(async () => {
    const { runReactLifecycleTests } = await import("/test-results/react-harness.js");
    return runReactLifecycleTests(new TextEncoder().encode("ATOM      1  CA  ALA A   1       0.000   0.000   0.000  1.00 20.00           C\nEND\n"));
  });
  expect(labels).toHaveLength(5); expect(errors).toEqual([]);
});
