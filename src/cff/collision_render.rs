// src/cff/collision_render.rs

use image::{Rgba, RgbaImage};

pub fn render_collision_polygon(vertices: &[(i16, i16)], img_w: u32, img_h: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(img_w, img_h, Rgba([17, 18, 20, 255]));

    if vertices.is_empty() {
        return img;
    }

    // 1. Draw subtle background coordinate axes
    let cx = (img_w / 2) as i32;
    let cy = (img_h / 2) as i32;
    for x in 0..img_w {
        img.put_pixel(x, cy as u32, Rgba([35, 37, 41, 255]));
    }
    for y in 0..img_h {
        img.put_pixel(cx as u32, y, Rgba([35, 37, 41, 255]));
    }

    // 2. Compute bounding box
    let mut min_x = i16::MAX;
    let mut max_x = i16::MIN;
    let mut min_y = i16::MAX;
    let mut max_y = i16::MIN;

    for &(x, y) in vertices {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let span_x = (max_x as f32 - min_x as f32).max(10.0);
    let span_y = (max_y as f32 - min_y as f32).max(10.0);
    let pad = 12.0f32;
    let target_w = img_w as f32 - pad * 2.0;
    let target_h = img_h as f32 - pad * 2.0;

    let scale = (target_w / span_x).min(target_h / span_y);
    let mid_x = (min_x as f32 + max_x as f32) / 2.0;
    let mid_y = (min_y as f32 + max_y as f32) / 2.0;

    let to_screen = |x: i16, y: i16| -> (i32, i32) {
        let sx = cx as f32 + (x as f32 - mid_x) * scale;
        let sy = cy as f32 - (y as f32 - mid_y) * scale;
        (sx.round() as i32, sy.round() as i32)
    };

    let screen_pts: Vec<(i32, i32)> = vertices.iter().map(|&(x, y)| to_screen(x, y)).collect();
    let num_pts = screen_pts.len();

    // 3. Draw polygon edges via Bresenham's algorithm
    let edge_color = Rgba([0, 255, 204, 255]); // Bright neon cyan/green
    for i in 0..num_pts {
        let (x0, y0) = screen_pts[i];
        let (x1, y1) = screen_pts[(i + 1) % num_pts];
        draw_line_segment(&mut img, x0, y0, x1, y1, edge_color);
    }

    // 4. Draw vertex dots
    let dot_color = Rgba([240, 177, 50, 255]); // Gold points
    for &(px, py) in &screen_pts {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let nx = px + dx;
                let ny = py + dy;
                if nx >= 0 && nx < img_w as i32 && ny >= 0 && ny < img_h as i32 {
                    img.put_pixel(nx as u32, ny as u32, dot_color);
                }
            }
        }
    }

    img
}

fn draw_line_segment(
    img: &mut RgbaImage,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (w, h) = (img.width() as i32, img.height() as i32);

    loop {
        if x0 >= 0 && x0 < w && y0 >= 0 && y0 < h {
            img.put_pixel(x0 as u32, y0 as u32, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}
