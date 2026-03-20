//! QR code → egui texture conversion.

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// Generate a QR code texture from a URI string.
/// Each QR module is rendered as `pixel_size x pixel_size` pixels.
pub fn generate_qr_texture(ctx: &Context, uri: &str) -> Option<TextureHandle> {
    let code = qrcode::QrCode::new(uri.as_bytes()).ok()?;
    let module_count = code.width();
    let pixel_size: usize = 6;
    let quiet_zone: usize = 4;
    let image_modules = module_count + quiet_zone * 2;
    let image_size = image_modules * pixel_size;

    let mut pixels = vec![255u8; image_size * image_size * 3];

    for y in 0..module_count {
        for x in 0..module_count {
            let is_dark = code[(x, y)] == qrcode::Color::Dark;
            let color = if is_dark { 0u8 } else { 255u8 };
            for py in 0..pixel_size {
                for px in 0..pixel_size {
                    let row = (y + quiet_zone) * pixel_size + py;
                    let col = (x + quiet_zone) * pixel_size + px;
                    let idx = (row * image_size + col) * 3;
                    pixels[idx] = color;
                    pixels[idx + 1] = color;
                    pixels[idx + 2] = color;
                }
            }
        }
    }

    let image = ColorImage::from_rgb([image_size, image_size], &pixels);
    Some(ctx.load_texture("qr_code", image, TextureOptions::NEAREST))
}
