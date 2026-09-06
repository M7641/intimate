use wasm_bindgen::prelude::*;

/// Renders an RGBA checkerboard, returning `width * height * 4` bytes.
///
/// A square is "dark" when `x + y` is even. Dark squares use `dark`, light
/// squares use `light` (both grayscale); every pixel is fully opaque. The
/// returned buffer can be handed straight to a canvas `ImageData`.
#[wasm_bindgen]
pub fn color_checkerboard(width: usize, height: usize, dark: u8, light: u8) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let shade = if (x + y) % 2 == 0 { dark } else { light };
            buffer.extend_from_slice(&[shade, shade, shade, 255]);
        }
    }

    buffer
}
