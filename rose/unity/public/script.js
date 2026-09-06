import init, { color_checkerboard } from "./wasm/suffering.js";

await init();

const SIZE = 12;   // checkerboard is SIZE x SIZE squares
const SCALE = 20;  // each square is drawn SCALE px wide

const canvas = document.getElementById("checkerboard");
canvas.width = SIZE;
canvas.height = SIZE;
// Render at native resolution, then upscale with CSS for crisp pixels.
canvas.style.width = `${SIZE * SCALE}px`;
canvas.style.height = `${SIZE * SCALE}px`;
canvas.style.imageRendering = "pixelated";

const ctx = canvas.getContext("2d");
const image = ctx.createImageData(SIZE, SIZE);

let inverted = false;
const draw = () => {
    const [dark, light] = inverted ? [0, 255] : [255, 0];
    image.data.set(color_checkerboard(SIZE, SIZE, dark, light));
    ctx.putImageData(image, 0, 0);
    inverted = !inverted;
};

draw();
setInterval(draw, 2500); // flip the colours every 2.5s so the demo is alive
