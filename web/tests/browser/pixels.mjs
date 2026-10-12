export async function screenshotColors(page) {
  const screenshot = await page.locator("#viewer canvas").screenshot();
  return await page.evaluate(async bytes => {
    const bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d");
    context.drawImage(bitmap, 0, 0);
    const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
    let red = 0, blue = 0;
    for (let i = 0; i < pixels.length; i += 4) {
      const [r, g, b] = pixels.subarray(i, i + 3);
      if (r > b + 35 && r > g + 25) red++;
      if (b > r + 15 && b > g + 3) blue++;
    }
    return { red, blue };
  }, [...screenshot]);
}
