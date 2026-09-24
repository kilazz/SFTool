// src/terrain/generator.rs

use rayon::prelude::*;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct MapGenConfig {
    pub width: usize,
    pub height: usize,
    pub base_z: u16,
    pub cell_size_x: usize,
    pub cell_size_y: usize,
    pub erosion_mean_x: f32,
    pub erosion_sigma_x: f32,
    pub erosion_mean_y: f32,
    pub erosion_sigma_y: f32,
    pub blur_size: usize,
    pub blur_sigma: f32,
}

impl Default for MapGenConfig {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            base_z: 2000,
            cell_size_x: 16,
            cell_size_y: 16,
            erosion_mean_x: 3.0,
            erosion_sigma_x: 1.0,
            erosion_mean_y: 3.0,
            erosion_sigma_y: 1.0,
            blur_size: 2,
            blur_sigma: 1.2,
        }
    }
}

pub struct TerrainGenerator;

impl TerrainGenerator {
    /// Generates a raw 16-bit heightmap array using parallel cellular gradient erosion and smoothing.
    pub fn generate_heightmap(cfg: &MapGenConfig) -> Vec<u16> {
        let total_pixels = cfg.width * cfg.height;
        let mut raw_float = vec![0.0f32; total_pixels];

        // 1. Parallel cellular gradient synthesis
        raw_float
            .par_chunks_mut(cfg.width)
            .enumerate()
            .for_each(|(y, row)| {
                for (x, val) in row.iter_mut().enumerate() {
                    let cell_x = (x / cfg.cell_size_x) as f32;
                    let cell_y = (y / cfg.cell_size_y) as f32;

                    let fx = (x % cfg.cell_size_x) as f32 / cfg.cell_size_x as f32;
                    let fy = (y % cfg.cell_size_y) as f32 / cfg.cell_size_y as f32;

                    let grad_x = (cell_x * 12.9898 + cell_y * 78.233).sin() * 43_758.547_f32;
                    let grad_y = (cell_x * 39.3461 + cell_y * 11.135).sin() * 23_421.63_f32;

                    let ridge_x = (grad_x.fract() * cfg.erosion_sigma_x + cfg.erosion_mean_x)
                        * (fx * std::f32::consts::PI).sin();
                    let ridge_y = (grad_y.fract() * cfg.erosion_sigma_y + cfg.erosion_mean_y)
                        * (fy * std::f32::consts::PI).sin();

                    *val = cfg.base_z as f32 + (ridge_x * ridge_y * 150.0);
                }
            });

        // 2. Parallel Gaussian / Box blur smoothing pass
        let mut smoothed = raw_float.clone();
        let b = cfg.blur_size;
        if b > 0 {
            smoothed
                .par_chunks_mut(cfg.width)
                .enumerate()
                .for_each(|(y, row)| {
                    for (x, pixel) in row.iter_mut().enumerate() {
                        let mut sum = 0.0f32;
                        let mut count = 0.0f32;

                        for dy in -(b as isize)..=(b as isize) {
                            for dx in -(b as isize)..=(b as isize) {
                                let nx = x as isize + dx;
                                let ny = y as isize + dy;
                                if nx >= 0
                                    && nx < cfg.width as isize
                                    && ny >= 0
                                    && ny < cfg.height as isize
                                {
                                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                                    let weight =
                                        (-dist / (2.0 * cfg.blur_sigma * cfg.blur_sigma)).exp();
                                    sum +=
                                        raw_float[ny as usize * cfg.width + nx as usize] * weight;
                                    count += weight;
                                }
                            }
                        }
                        if count > 0.0 {
                            *pixel = sum / count;
                        }
                    }
                });
        }

        smoothed
            .into_iter()
            .map(|v| v.clamp(0.0, 65535.0).round() as u16)
            .collect()
    }

    /// Exports the heightmap into a standard 16-bit grayscale PNG image.
    pub fn export_png_16bit(
        heights: &[u16],
        width: u32,
        height: u32,
        out_path: &Path,
    ) -> io::Result<()> {
        let mut img_buf = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                img_buf.put_pixel(x, y, image::Luma([heights[idx]]));
            }
        }
        img_buf
            .save(out_path)
            .map_err(|e| io::Error::other(e.to_string()))
    }

    /// Exports raw uncompressed Little-Endian height values.
    pub fn export_raw_heightmap(heights: &[u16], out_path: &Path) -> io::Result<()> {
        let mut f = File::create(out_path)?;
        for &h in heights {
            f.write_all(&h.to_le_bytes())?;
        }
        Ok(())
    }
}
