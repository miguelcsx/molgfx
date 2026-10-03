// Use full Chromium: headless-shell has unreliable WebGPU lifecycle on macOS.
// Linux SwiftShader also needs the headed compositor (run under xvfb-run in CI);
// headless Chromium can submit valid GPU work while presenting a blank canvas.
export const browserLaunchOptions = {
  channel: "chromium",
  headless: process.platform !== "linux",
  args:
    process.platform === "linux"
      ? [
          "--enable-unsafe-webgpu",
          "--use-angle=swiftshader",
          "--enable-features=Vulkan",
          "--use-vulkan=swiftshader",
          "--disable-vulkan-surface",
        ]
      : ["--enable-unsafe-webgpu"],
};
