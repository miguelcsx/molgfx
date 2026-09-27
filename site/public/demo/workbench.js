const examplePdb = [
  "ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00 20.00           N",
  "ATOM      2  CA  ALA A   1       1.458   0.000   0.000  1.00 20.00           C",
  "ATOM      3  C   ALA A   1       2.008   1.410   0.000  1.00 20.00           C",
  "ATOM      4  N   ALA A   2       3.358   1.640   0.420  1.00 20.00           N",
  "ATOM      5  CA  ALA A   2       3.988   2.964   0.510  1.00 20.00           C",
  "ATOM      6  C   ALA A   2       3.220   3.880   1.470  1.00 20.00           C",
  "ATOM      7  N   ALA A   3       3.862   5.070   1.810  1.00 20.00           N",
  "ATOM      8  CA  ALA A   3       3.260   6.018   2.760  1.00 20.00           C",
  "ATOM      9  C   ALA A   3       1.816   6.330   2.390  1.00 20.00           C",
  "ATOM     10  N   ALA A   4       1.340   7.606   2.470  1.00 20.00           N",
  "ATOM     11  CA  ALA A   4      -0.046   7.932   2.170  1.00 20.00           C",
  "ATOM     12  C   ALA A   4      -0.836   6.878   1.390  1.00 20.00           C",
  "ATOM     13  N   ALA A   5      -2.168   7.150   1.170  1.00 20.00           N",
  "ATOM     14  CA  ALA A   5      -3.038   6.194   0.460  1.00 20.00           C",
  "ATOM     15  C   ALA A   5      -2.538   4.765   0.740  1.00 20.00           C",
  "ATOM     16  N   ALA A   6      -3.492   3.900   0.280  1.00 20.00           N",
  "ATOM     17  CA  ALA A   6      -3.136   2.520   0.290  1.00 20.00           C",
  "ATOM     18  C   ALA A   6      -1.750   2.218  -0.270  1.00 20.00           C",
  "ATOM     19  N   ALA A   7      -1.430   0.900  -0.400  1.00 20.00           N",
  "ATOM     20  CA  ALA A   7      -0.120   0.508  -0.910  1.00 20.00           C",
  "ATOM     21  C   ALA A   7       0.640   1.624  -1.580  1.00 20.00           C",
  "TER",
  "END",
].join("\n");

const initialProgram = "show cartoon, protein; color chain, @cartoon; show spacefill, ligand";

// The structure the demo opens with: oxyhaemoglobin, fetched from the RCSB
// by the visitor's browser. Offline, or if the archive is unreachable, the
// small helix above stands in so the demo still starts.
const defaultStructure = {
  url: "https://files.rcsb.org/download/1HHO.cif",
  name: "1hho.cif",
};

// The most device pixels a frame renders: every full-screen pass scales with
// it, and a wide canvas on a high-density display is 7-8 million pixels.
const PIXEL_BUDGET = 2560 * 1600;

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
    camera.target[2] + horizontal * Math.cos(yaw),
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
  const label = create("label", "molgfx-demo-file");
  const fileLabel = create("span", "molgfx-demo-file-label", "Open structure");
  const fileHint = create("span", "molgfx-demo-file-hint", "PDB · mmCIF · BCIF");
  const file = document.createElement("input");
  file.type = "file";
  file.accept = ".cif,.mmcif,.bcif,.pdb";
  file.setAttribute("aria-label", "Choose a molecular structure file");
  label.append(fileLabel, fileHint, file);
  const status = create("output", "molgfx-demo-status", "Loading example…");
  toolbar.append(label, status);

  const canvas = create("canvas", "molgfx-demo-canvas");
  const consolePanel = create("form", "molgfx-demo-console");
  const command = create("input", "molgfx-demo-command");
  command.type = "text";
  command.spellcheck = false;
  command.autocomplete = "off";
  command.placeholder = "e.g. color chain, all";
  const run = create("button", "molgfx-demo-run", "Apply");
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
    const cssWidth = Math.max(1, canvas.clientWidth);
    const cssHeight = Math.max(1, canvas.clientHeight);
    const ratio = Math.min(window.devicePixelRatio || 1, Math.sqrt(PIXEL_BUDGET / (cssWidth * cssHeight)));
    const width = Math.max(1, Math.round(cssWidth * ratio));
    const height = Math.max(1, Math.round(cssHeight * ratio));
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

  // A failed frame is reported, never left as a blank canvas. GPU validation
  // errors arrive after the frame that caused them and surface on the next
  // one, so every frame is followed by one quiet check frame.
  let settling;
  // A pick holds the renderer across its GPU readback; a frame requested
  // meanwhile would reenter it, so it is drawn once the pick completes.
  let picking = false;
  let redrawAfterPick = false;
  const frame = (followUp = true) => {
    if (picking) {
      redrawAfterPick = true;
      return;
    }
    try {
      draw();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      details.textContent = message;
      report(`MolGFX could not draw: ${message}`, true);
      return;
    }
    if (followUp) {
      clearTimeout(settling);
      settling = setTimeout(() => frame(false), 250);
    }
  };

  const execute = (text) => {
    const answer = JSON.parse(session.execute(scene, text));
    if (!answer.ok) {
      details.textContent = answer.rendered || answer.errors?.map((error) => error.message).join("\n") || "Command failed.";
      report("Command failed. Read the diagnostic below.", true);
      return;
    }
    details.textContent = answer.messages?.join("\n") || "";
    report("Structure ready.");
    frame();
  };

  const loadBytes = async (bytes, name) => {
    try {
      report(`Loading ${name}…`);
      await requireWebGPU();
      runtime ??= await import("./molgfx_wasm.js");
      await runtime.default();
      scene = runtime.Scene.fromStructureBytes(bytes, name);
      renderer ??= await runtime.Renderer.create(canvas);
      session = new runtime.Session(scene);
      camera = cameraFor(scene, canvas);
      execute(initialProgram);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      details.textContent = message;
      report(message, true);
    }
  };

  const loadSelected = async () => {
    const selected = file.files?.[0];
    if (!selected) return;
    file.disabled = true;
    try {
      await loadBytes(new Uint8Array(await selected.arrayBuffer()), selected.name);
    } finally {
      file.disabled = false;
    }
  };

  // Input only requests a frame; one is drawn per display refresh, so a busy
  // GPU never accumulates a queue of frames behind a drag.
  let scheduled = 0;
  const requestFrame = () => {
    if (scheduled === 0) {
      scheduled = requestAnimationFrame(() => {
        scheduled = 0;
        frame();
      });
    }
  };

  const resize = new ResizeObserver(() => requestFrame());
  resize.observe(canvas);
  file.addEventListener("change", loadSelected);
  const loadDefault = async () => {
    try {
      report("Fetching 1HHO from the RCSB…");
      const response = await fetch(defaultStructure.url, {signal: AbortSignal.timeout(15000)});
      if (!response.ok) throw new Error(`the RCSB answered ${response.status}`);
      await loadBytes(new Uint8Array(await response.arrayBuffer()), defaultStructure.name);
    } catch {
      await loadBytes(new TextEncoder().encode(examplePdb), "alanine-helix.pdb");
    }
  };
  void loadDefault();
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
    requestFrame();
  });
  canvas.addEventListener("pointerup", (event) => {
    pointer = undefined;
    canvas.releasePointerCapture(event.pointerId);
  });
  canvas.addEventListener("wheel", (event) => {
    if (!camera) return;
    event.preventDefault();
    zoom(camera, event.deltaY);
    requestFrame();
  }, { passive: false });
  canvas.addEventListener("click", async (event) => {
    if (!renderer || picking) return;
    const bounds = canvas.getBoundingClientRect();
    picking = true;
    try {
      const result = await renderer.pick(
        Math.floor((event.clientX - bounds.left) * canvas.width / bounds.width),
        Math.floor((event.clientY - bounds.top) * canvas.height / bounds.height),
      );
      details.textContent = result ?? "Background";
    } catch (error) {
      report(`Picking failed: ${error instanceof Error ? error.message : String(error)}`, true);
    } finally {
      picking = false;
      if (redrawAfterPick) {
        redrawAfterPick = false;
        frame();
      }
    }
  });

  return () => {
    clearTimeout(settling);
    cancelAnimationFrame(scheduled);
    resize.disconnect();
    root.replaceChildren();
  };
}
