// The runtime is a wasm-bindgen ES module plus its WebAssembly binary. A
// notebook frontend loads this file from a blob URL, where a relative import
// has nothing to resolve against, so the kernel ships both parts as widget
// state and they are instantiated from memory. A page that serves this file
// beside the runtime (the browser tests) imports it relatively instead.
const runtimes = new Map();

export function loadRuntime(model) {
  const glue = model.get("_runtime_js");
  const key = glue ? model.get("_runtime_key") || "inline" : "static";
  if (!runtimes.has(key)) {
    runtimes.set(key, (async () => {
      if (glue) {
        const url = URL.createObjectURL(new Blob([glue], {type: "text/javascript"}));
        try {
          const runtime = await import(url);
          await runtime.default({module_or_path: sourceBytes(model.get("_runtime_wasm"))});
          return runtime;
        } finally {
          URL.revokeObjectURL(url);
        }
      }
      const runtime = await import("./molgfx_wasm.js");
      await runtime.default({module_or_path: new URL("./molgfx_wasm_bg.wasm", import.meta.url)});
      return runtime;
    })());
  }
  return runtimes.get(key);
}

function dimensions(canvas) {
  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
  const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
  return [width, height];
}

function sourceBytes(value) {
  return value instanceof Uint8Array ? value : new Uint8Array(value.buffer || value);
}

async function buildScene(model, {Scene}) {
  const scene = new Scene(model.get("scene_spec"));
  const ids = model.get("structure_ids");
  const names = model.get("structure_names");
  const payloads = model.get("structure_payloads");
  if (ids.length !== names.length || ids.length !== payloads.length) {
    throw new Error("structure transport columns have different lengths");
  }
  for (let index = 0; index < ids.length; index += 1) {
    // The binding's identity parameter is a Rust `u64`, which wasm-bindgen
    // maps to a JavaScript BigInt rather than a Number, so the transported
    // integer has to be widened before the call.
    scene.bindStructure(BigInt(ids[index]), sourceBytes(payloads[index]), names[index]);
  }
  scene.resolve();
  return scene;
}

function requestSync(model) {
  model.set("sync_request", (model.get("sync_request") || 0) + 1);
  model.save_changes();
}

function report(model, error) {
  model.set("error", error instanceof Error ? error.message : String(error));
  model.save_changes();
}

function rotateCamera(camera, dx, dy) {
  const offset = camera.position.map((value, index) => value - camera.target[index]);
  const radius = Math.hypot(...offset);
  const yaw = Math.atan2(offset[0], offset[2]) - dx * 0.006;
  const pitch = Math.max(-1.5, Math.min(1.5, Math.asin(offset[1] / radius) + dy * 0.006));
  const horizontal = radius * Math.cos(pitch);
  camera.position = [
    camera.target[0] + horizontal * Math.sin(yaw),
    camera.target[1] + radius * Math.sin(pitch),
    camera.target[2] + horizontal * Math.cos(yaw),
  ];
}

function zoomCamera(camera, delta) {
  const offset = camera.position.map((value, index) => value - camera.target[index]);
  const scale = Math.exp(Math.max(-1, Math.min(1, delta * 0.001)));
  camera.position = camera.target.map((value, index) => value + offset[index] * scale);
}

function publishCamera(model, camera) {
  model.set("camera", {
    position: camera.position,
    target: camera.target,
    up: camera.up,
  });
  model.save_changes();
}

async function mount({model, el}) {
  const canvas = document.createElement("canvas");
  canvas.className = "molgfx-canvas";
  el.appendChild(canvas);
  const detach = model.get("workbench") ? mountConsole(model, el) : () => {};
  const runtime = await loadRuntime(model);
  const {Renderer, ScenePatch} = runtime;

  let scene = await buildScene(model, runtime);
  const renderer = await Renderer.create(canvas);
  // Patches that went by before this view mounted are not replayed; a view
  // that is behind asks the kernel for the current specification instead.
  const behind = () => Number(scene.revision) !== model.get("revision");
  let camera;
  const draw = () => {
    const [width, height] = dimensions(canvas);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      renderer.resize(width, height);
    }
    if (camera === undefined) {
      camera = JSON.parse(scene.cameraJSON(width, height));
    }
    renderer.renderCamera(
      scene,
      new Float32Array(camera.position),
      new Float32Array(camera.target),
      new Float32Array(camera.up),
    );
  };

  const resizeObserver = new ResizeObserver(() => {
    try { draw(); } catch (error) { report(model, error); }
  });
  resizeObserver.observe(canvas);

  const replace = async () => {
    try {
      scene = await buildScene(model, runtime);
      camera = undefined;
      draw();
    } catch (error) {
      report(model, error);
    }
  };
  const patch = () => {
    try {
      const encoded = model.get("scene_patch");
      if (JSON.parse(encoded).base_revision !== Number(scene.revision)) {
        requestSync(model);
        return;
      }
      scene.apply(new ScenePatch(encoded));
      const operations = JSON.parse(encoded).operations;
      if (operations.some((operation) => operation.op === "set_camera")) {
        camera = undefined;
      }
      draw();
    } catch (error) {
      report(model, error);
    }
  };

  model.on("change:scene_spec", replace);
  model.on("change:patch_sequence", patch);
  const pick = async (event) => {
    try {
      const bounds = canvas.getBoundingClientRect();
      const x = Math.floor((event.clientX - bounds.left) * canvas.width / bounds.width);
      const y = Math.floor((event.clientY - bounds.top) * canvas.height / bounds.height);
      const result = await renderer.pick(x, y);
      model.set("pick", result === undefined ? {} : JSON.parse(result));
      model.set("selection", result === undefined ? "" : result);
      model.save_changes();
    } catch (error) {
      report(model, error);
    }
  };
  let pointer;
  const pointerDown = (event) => {
    pointer = [event.clientX, event.clientY];
    canvas.setPointerCapture(event.pointerId);
  };
  const pointerMove = (event) => {
    if (pointer === undefined) return;
    rotateCamera(camera, event.clientX - pointer[0], event.clientY - pointer[1]);
    pointer = [event.clientX, event.clientY];
    draw();
  };
  const pointerUp = (event) => {
    if (pointer === undefined) return;
    pointer = undefined;
    canvas.releasePointerCapture(event.pointerId);
    publishCamera(model, camera);
  };
  const wheel = (event) => {
    event.preventDefault();
    zoomCamera(camera, event.deltaY);
    draw();
    publishCamera(model, camera);
  };
  canvas.addEventListener("click", pick);
  canvas.addEventListener("pointerdown", pointerDown);
  canvas.addEventListener("pointermove", pointerMove);
  canvas.addEventListener("pointerup", pointerUp);
  canvas.addEventListener("wheel", wheel, {passive: false});
  draw();
  if (behind()) requestSync(model);

  return () => {
    detach();
    resizeObserver.disconnect();
    model.off("change:scene_spec", replace);
    model.off("change:patch_sequence", patch);
    canvas.removeEventListener("click", pick);
    canvas.removeEventListener("pointerdown", pointerDown);
    canvas.removeEventListener("pointermove", pointerMove);
    canvas.removeEventListener("pointerup", pointerUp);
    canvas.removeEventListener("wheel", wheel);
  };
}

/// Mounts the viewer, and makes any failure visible.
///
/// A failure while starting — no WebGPU adapter, a runtime that will not
/// instantiate, a structure that will not decode — would otherwise leave an
/// empty canvas, with the reason only in the browser console. It is written
/// into the output and into the `error` trait, where the kernel can read it.
async function render({model, el}) {
  try {
    return await mount({model, el});
  } catch (error) {
    report(model, error);
    const message = element("pre", "molgfx-failure", `MolGFX viewer could not start: ${error instanceof Error ? error.message : String(error)}`);
    el.appendChild(message);
    throw error;
  }
}

export default {render};

// ---------------------------------------------------------------------------
// Command console (Workbench only)
// ---------------------------------------------------------------------------

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/// A command line, history and error panel under the canvas.
///
/// Requests travel as the `command_request` trait and answers come back as
/// `command_reply`; commands run in the kernel, and their effect reaches the
/// canvas as ordinary scene patches, never as pixels.
function mountConsole(model, el) {
  const panel = element("div", "molgfx-console");
  const log = element("ol", "molgfx-log");
  const row = element("div", "molgfx-prompt");
  const prompt = element("span", "molgfx-caret", "›");
  const input = element("input", "molgfx-input");
  input.type = "text";
  input.spellcheck = false;
  input.autocomplete = "off";
  input.placeholder = "show cartoon, protein   (Tab completes, ↑↓ history)";
  const menu = element("ul", "molgfx-completions");
  const status = element("pre", "molgfx-status");
  row.append(prompt, input);
  panel.append(log, row, menu, status);
  el.appendChild(panel);

  let sequence = 0;
  let recall = -1;
  const pending = new Map();

  const request = (payload) => {
    sequence += 1;
    const id = `${Date.now()}-${sequence}`;
    pending.set(id, payload.type);
    model.set("command_request", {...payload, id});
    model.save_changes();
    return id;
  };

  const entry = (text, ok, detail) => {
    const item = element("li", ok ? "molgfx-ok" : "molgfx-failed");
    item.append(element("code", "", text));
    if (detail) item.append(element("div", "molgfx-detail", detail));
    log.append(item);
    log.scrollTop = log.scrollHeight;
  };

  const showCompletions = (items) => {
    menu.replaceChildren();
    if (items.length === 1) {
      replaceWord(items[0].text);
      return;
    }
    for (const item of items.slice(0, 12)) {
      const option = element("li", "", item.text);
      option.title = item.detail || item.kind;
      option.addEventListener("mousedown", (event) => {
        event.preventDefault();
        replaceWord(item.text);
        menu.replaceChildren();
      });
      menu.append(option);
    }
  };

  const replaceWord = (text) => {
    const cursor = input.selectionStart ?? input.value.length;
    const before = input.value.slice(0, cursor);
    const start = Math.max(before.lastIndexOf(" "), before.lastIndexOf(","), before.lastIndexOf(";")) + 1;
    input.value = before.slice(0, start) + text + input.value.slice(cursor);
    const caret = start + text.length;
    input.setSelectionRange(caret, caret);
    input.focus();
  };

  const reply = () => {
    const answer = model.get("command_reply") || {};
    const kind = pending.get(answer.id);
    if (kind === undefined) return;
    pending.delete(answer.id);
    if (answer.type === "completions") {
      showCompletions(answer.items || []);
      return;
    }
    if (answer.type !== "result") return;
    if (answer.ok) {
      entry(answer.text, true, (answer.messages || []).join("\n"));
      status.textContent = `revision ${answer.revision}`;
      status.className = "molgfx-status";
    } else {
      entry(answer.text, false, (answer.errors || []).map((error) => error.message).join("\n"));
      status.textContent = answer.rendered || "";
      status.className = "molgfx-status molgfx-error";
    }
  };

  const keydown = (event) => {
    const history = model.get("history") || [];
    if (event.key === "Enter" && input.value.trim() !== "") {
      event.preventDefault();
      menu.replaceChildren();
      request({type: "execute", text: input.value});
      input.value = "";
      recall = -1;
    } else if (event.key === "Tab") {
      event.preventDefault();
      request({type: "complete", text: input.value, cursor: input.selectionStart ?? input.value.length});
    } else if (event.key === "ArrowUp" && history.length > 0) {
      event.preventDefault();
      recall = recall < 0 ? history.length - 1 : Math.max(0, recall - 1);
      input.value = history[recall];
    } else if (event.key === "ArrowDown" && recall >= 0) {
      event.preventDefault();
      recall += 1;
      input.value = recall < history.length ? history[recall] : "";
      if (recall >= history.length) recall = -1;
    } else if (event.key === "Escape") {
      menu.replaceChildren();
    }
  };

  // Keys typed into the console belong to the console, not the notebook.
  const isolate = (event) => event.stopPropagation();
  input.addEventListener("keydown", keydown);
  input.addEventListener("keydown", isolate);
  input.addEventListener("keypress", isolate);
  input.addEventListener("keyup", isolate);
  model.on("change:command_reply", reply);

  return () => {
    model.off("change:command_reply", reply);
    input.removeEventListener("keydown", keydown);
    input.removeEventListener("keydown", isolate);
    input.removeEventListener("keypress", isolate);
    input.removeEventListener("keyup", isolate);
    panel.remove();
  };
}
