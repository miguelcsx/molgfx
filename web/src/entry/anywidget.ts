import { ViewerCore } from "../runtime/core.js";
import { AnywidgetAdapter } from "../anywidget/adapter.js";
import type { RenderContext } from "../anywidget/model.js";
async function render({ model, el }: RenderContext): Promise<() => void> {
  const adapter = new AnywidgetAdapter(model);
  const viewer = new ViewerCore(
    el,
    {},
    { el, source: adapter, sink: adapter, inlineRuntime: adapter },
  );
  await viewer.initialize({});
  return () => {
    void viewer.dispose();
  };
}
export default { render };
