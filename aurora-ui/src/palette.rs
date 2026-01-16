use anyhow::{Result, Context};
use image::GenericImageView;
use palette::Srgb;
use std::path::Path;

pub struct ThemePalette {
    pub background: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
}

pub fn extract_palette<P: AsRef<Path>>(path: P) -> Result<ThemePalette> {
    let img = image::open(path).context("Failed to open image")?;
    let (width, height) = img.dimensions();
    
    // Simple dominant color approach: sample 10x10 grid
    let mut colors = Vec::new();
    let step_x = (width / 10).max(1);
    let step_y = (height / 10).max(1);

    for x in (0..width).step_by(step_x as usize) {
        for y in (0..height).step_by(step_y as usize) {
            let pixel = img.get_pixel(x, y);
            colors.push(Srgb::new(
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            ));
        }
    }

    // Sort by "vibrancy" or just pick the average
    // For now, let's just pick a few representative ones
    let primary = colors[colors.len() / 2];
    let background = darken(&primary, 0.2);
    let secondary = lighten(&primary, 0.8);
    let accent = primary; // Or a complementary color

    Ok(ThemePalette {
        background: color_to_hex(background),
        primary: color_to_hex(primary),
        secondary: color_to_hex(secondary),
        accent: color_to_hex(accent),
    })
}

fn darken(color: &Srgb<f32>, factor: f32) -> Srgb<f32> {
    Srgb::new(color.red * factor, color.green * factor, color.blue * factor)
}

fn lighten(color: &Srgb<f32>, factor: f32) -> Srgb<f32> {
    Srgb::new(
        color.red + (1.0 - color.red) * factor,
        color.green + (1.0 - color.green) * factor,
        color.blue + (1.0 - color.blue) * factor,
    )
}

fn color_to_hex(color: Srgb<f32>) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (color.red * 255.0) as u8,
        (color.green * 255.0) as u8,
        (color.blue * 255.0) as u8
    )
}
