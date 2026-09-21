import init, {Renderer, Scene, ScenePatch} from "./molgfx_wasm.js";

function dimensions(canvas) {
  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.round(canvas.clientWidth * ratio));
  const height = Math.max(1, Math.round(canvas.clientHeight * ratio));
  return [width, height];
}

function sourceBytes(value) {
  return value instanceof Uint8Array ? value : new Uint8Array(value.buffer || value);
}

async function buildScene(model) {
  const scene = new Scene(model.get("scene_spec"));
  const ids = model.get("structure_ids");
  const names = model.get("structure_names");
  const payloads = model.get("structure_payloads");
  if (ids.length !== names.length || ids.length !== payloads.length) {
    throw new Error("structure transport columns have different lengths");
  }
  for (let index = 0; index < ids.length; index += 1) {
    scene.bindStructure(ids[index], sourceBytes(payloads[index]), names[index]);
  }
  scene.resolve();
  return scene;
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

export async function render({model, el}) {
  const canvas = document.createElement("canvas");
  canvas.className = "molgfx-canvas";
  el.appendChild(canvas);
  await init(new URL("./molgfx_wasm_bg.wasm", import.meta.url));

  let scene = await buildScene(model);
  const renderer = await Renderer.create(canvas);
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
      scene = await buildScene(model);
      camera = undefined;
      draw();
    } catch (error) {
      report(model, error);
    }
  };
  const patch = () => {
    try {
      const encoded = model.get("scene_patch");
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

  return () => {
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
