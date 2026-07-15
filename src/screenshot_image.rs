// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! Off-thread decoding and downscaling of screenshot images.
//!
//! Adapted from cosmic-files' `large_image.rs`: raster screenshots are decoded
//! to RGBA and downscaled (Lanczos3) to roughly the on-screen display size, so
//! the UI thread never decodes a multi-megapixel PNG mid-frame when the gallery
//! switches images.

use std::io::Cursor;

/// Decode at up to this multiple of the display size, so the image still looks
/// crisp if the window grows a little or the GPU upscales slightly.
const DISPLAY_SCALE_FACTOR: f32 = 1.5;

fn calculate_target_dimensions(
    image_width: u32,
    image_height: u32,
    display_width: u32,
    display_height: u32,
) -> Option<(u32, u32)> {
    let target_width = (display_width as f32 * DISPLAY_SCALE_FACTOR) as u32;
    let target_height = (display_height as f32 * DISPLAY_SCALE_FACTOR) as u32;

    if target_width == 0 || target_height == 0 {
        return None;
    }
    if image_width <= target_width && image_height <= target_height {
        return None;
    }

    let image_aspect = image_width as f32 / image_height as f32;
    let target_aspect = target_width as f32 / target_height as f32;

    let (new_width, new_height) = if image_aspect > target_aspect {
        (target_width, (target_width as f32 / image_aspect) as u32)
    } else {
        ((target_height as f32 * image_aspect) as u32, target_height)
    };

    Some((new_width.max(1), new_height.max(1)))
}

pub fn decode_scaled(
    data: &[u8],
    display_width: u32,
    display_height: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = reader.decode().ok()?;
    let rgba = img.into_rgba8();
    let (orig_width, orig_height) = (rgba.width(), rgba.height());

    match calculate_target_dimensions(orig_width, orig_height, display_width, display_height) {
        Some((target_w, target_h)) => {
            // Lanczos3 for high-quality downsampling
            let resized = image::imageops::resize(
                &rgba,
                target_w,
                target_h,
                image::imageops::FilterType::Lanczos3,
            );
            let (w, h) = (resized.width(), resized.height());
            Some((w, h, resized.into_raw()))
        }
        None => Some((orig_width, orig_height, rgba.into_raw())),
    }
}
