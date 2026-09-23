use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub fn rgba_to_slint(rgba: image::RgbaImage) -> Image {
    let (width, height) = rgba.dimensions();
    let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    pixel_buffer.make_mut_bytes().copy_from_slice(rgba.as_raw());
    Image::from_rgba8(pixel_buffer)
}
