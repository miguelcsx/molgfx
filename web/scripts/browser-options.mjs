// The full Chromium channel uses the modern compositor; headless-shell on macOS
// cannot reliably deliver resize frames or keep multiple WebGPU devices alive.
export const browserLaunchOptions = {
  channel: "chromium",
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
