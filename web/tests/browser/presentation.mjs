import { expect } from "@playwright/test";

// The render loop requests an animation frame for as long as the renderer
// reports another presentation sample is due, including frames it skips
// while a slow adapter still owns the previous submission. No frame request
// outstanding, after the queue has drained, therefore means the image has
// converged. A quiet interval does not: SwiftShader can spend longer than
// any fixed window on one sample, so a timing heuristic reads mid-way
// accumulation as settled.
function installPresentationProbe() {
  const request = window.requestAnimationFrame.bind(window);
  const cancel = window.cancelAnimationFrame.bind(window);
  const pending = new Set();
  let submits = 0, queue;
  window.requestAnimationFrame = callback => {
    const handle = request(time => { pending.delete(handle); callback(time); });
    pending.add(handle);
    return handle;
  };
  window.cancelAnimationFrame = handle => { pending.delete(handle); cancel(handle); };
  const submit = GPUQueue.prototype.submit;
  GPUQueue.prototype.submit = function (...args) { submits++; queue = this; return submit.apply(this, args); };
  const tick = () => new Promise(resolve => request(() => resolve()));
  window.presentation = {
    submits: () => submits,
    async idle() {
      const start = submits;
      if (pending.size || !queue) return false;
      await queue.onSubmittedWorkDone();
      // A pick readback resolves after its work is done and may request a frame.
      await tick(); await tick();
      return pending.size === 0 && submits === start;
    },
  };
}

/** Instruments the page before any script runs; call before `page.goto`. */
export function trackPresentation(page) {
  return page.addInitScript(installPresentationProbe);
}

/** Waits until the render loop stops sampling and the GPU queue is drained. */
export function settled(page) {
  return expect.poll(() => page.evaluate(() => window.presentation.idle()), { timeout: 60000 }).toBe(true);
}

/** Presentation and readback submissions since the page loaded. */
export function submits(page) {
  return page.evaluate(() => window.presentation.submits());
}
