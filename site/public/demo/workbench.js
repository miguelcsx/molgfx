const initialProgram = "show cartoon, protein; color chain, @cartoon";

function create(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

async function requireWebGPU() {
  if (!("gpu" in navigator)) {
    throw new Error("WebGPU is unavailable in this browser. Use a current Chrome or Edge, or enable WebGPU in Safari or Firefox.");
  }
  if ((await navigator.gpu.requestAdapter()) === null) {
    throw new Error("WebGPU is available but this browser cannot open a compatible GPU adapter.");
  }
}

function cameraFor(scene, canvas) {
  return JSON.parse(scene.cameraJSON(canvas.width, canvas.height));
}

function rotate(camera, dx, dy) {
  const offset = camera.position.map((value, index) => value - camera.target[index]);
  const radius = Math.hypot(...offset);
  const yaw = Math.atan2(offset[0], offset[2]) - dx * 0.006;
  const pitch = Math.max(-1.5, Math.min(1.5, Math.asin(offset[1] / radius) + dy * 0.006));
  const horizontal = radius * Math.cos(pitch);
  camera.position = [
    camera.target[0] + horizontal * Math.sin(yaw),
    camera.target[1] + radius * Math.sin(pitch),
    camera.target[2] + horizontal * Math.cos(pitch),
  ];
}

function zoom(camera, delta) {
  const scale = Math.exp(Math.max(-1, Math.min(1, delta * 0.001)));
  camera.position = camera.position.map((value, index) => camera.target[index] + (value - camera.target[index]) * scale);
}

export function mount(root) {
  root.replaceChildren();
  const shell = create("section", "molgfx-demo");
  const toolbar = create("div", "molgfx-demo-toolbar");
  const label = create("label", "molgfx-demo-file", "Open local structure");
  const file = document.createElement("input");
  file.type = "file";
  file.accept = ".cif,.mmcif,.bcif,.pdb";
  label.append(file);
  const status = create("output", "molgfx-demo-status", "Choose a local mmCIF, BinaryCIF, or PDB file. The file stays in this browser.");
  toolbar.append(label, status);

  const canvas = create("canvas", "molgfx-demo-canvas");
  const consolePanel = create("form", "molgfx-demo-console");
  const command = create("input", "molgfx-demo-command");
  command.type = "text";
  command.spellcheck = false;
  command.autocomplete = "off";
  command.placeholder = "show cartoon, protein";
  const run = create("button", "molgfx-demo-run", "Run");
  run.type = "submit";
  const details = create("pre", "molgfx-demo-details");
  consolePanel.append(command, run, details);
  shell.append(toolbar, canvas, consolePanel);
  root.append(shell);

  let runtime;
  let scene;
  let renderer;
  let session;
  let camera;
  let pointer;
  const report = (message, isError = false) => {
    status.textContent = message;
    status.classList.toggle("is-error", isError);
  };

  const draw = () => {
    if (!renderer || !scene || !camera) return;
    const ratio = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
    const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      renderer.resize(width, height);
      camera = cameraFor(scene, canvas);
    }
    renderer.renderCamera(
      scene,
      new Float32Array(camera.position),
      new Float32Array(camera.target),
      new Float32Array(camera.up),
    );
  };

  const execute = (text) => {
    const answer = JSON.parse(session.execute(scene, text));
    if (!answer.ok) {
      details.textContent = answer.rendered || answer.errors?.map((error) => error.message).join("\n") || "Command failed.";
      report("Command failed. Read the diagnostic below.", true);
      return;
    }
    details.textContent = answer.messages?.join("\n") || `revision ${answer.revision}`;
    report(`Rendered revision ${answer.revision}.`);
    draw();
  };

  const load = async () => {
    const selected = file.files?.[0];
    if (!selected) return;
    try {
      file.disabled = true;
      report(`Loading ${selected.name}…`);
      await requireWebGPU();
      runtime ??= await import("./molgfx_wasm.js");
      await runtime.default();
      scene = runtime.Scene.fromStructureBytes(new Uint8Array(await selected.arrayBuffer()), selected.name);
      renderer ??= await runtime.Renderer.create(canvas);
      session = new runtime.Session(scene);
      camera = cameraFor(scene, canvas);
      execute(initialProgram);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      details.textContent = message;
      report(message, true);
    } finally {
      file.disabled = false;
    }
  };

  const resize = new ResizeObserver(() => {
    try { draw(); } catch (error) { report(String(error), true); }
  });
  resize.observe(canvas);
  file.addEventListener("change", load);
  consolePanel.addEventListener("submit", (event) => {
    event.preventDefault();
    if (session) execute(command.value);
  });
  command.addEventListener("keydown", (event) => {
    if (event.key !== "Tab" || !session) return;
    event.preventDefault();
    const items = JSON.parse(session.completions(command.value, command.selectionStart ?? command.value.length));
    if (items.length === 1) command.value = items[0].text;
    else details.textContent = items.slice(0, 8).map((item) => item.text).join("\n");
  });
  canvas.addEventListener("pointerdown", (event) => {
    if (!camera) return;
    pointer = [event.clientX, event.clientY];
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener("pointermove", (event) => {
    if (!pointer || !camera) return;
    rotate(camera, event.clientX - pointer[0], event.clientY - pointer[1]);
    pointer = [event.clientX, event.clientY];
    draw();
  });
  canvas.addEventListener("pointerup", (event) => {
    pointer = undefined;
    canvas.releasePointerCapture(event.pointerId);
  });
  canvas.addEventListener("wheel", (event) => {
    if (!camera) return;
    event.preventDefault();
    zoom(camera, event.deltaY);
    draw();
  }, { passive: false });
  canvas.addEventListener("click", async (event) => {
    if (!renderer) return;
    const bounds = canvas.getBoundingClientRect();
    const result = await renderer.pick(
      Math.floor((event.clientX - bounds.left) * canvas.width / bounds.width),
      Math.floor((event.clientY - bounds.top) * canvas.height / bounds.height),
    );
    details.textContent = result ?? "Background";
  });

  return () => {
    resize.disconnect();
    root.replaceChildren();
  };
}
