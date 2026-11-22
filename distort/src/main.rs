use image::{Rgba, RgbaImage};
use rand::Rng;
use std::env;
use std::path::Path;

fn clamp_u8(v: f32) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

fn idx(x: i32, y: i32, w: u32, h: u32) -> usize {
    let x = x.clamp(0, w as i32 - 1) as u32;
    let y = y.clamp(0, h as i32 - 1) as u32;
    (y * w + x) as usize
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: distort <input> [output]");
        std::process::exit(1);
    }

    let input = &args[1];
    let output = args.get(2).map(String::as_str).unwrap_or(input);

    let img = image::open(input)
        .unwrap_or_else(|e| {
            eprintln!("Failed to open {input}: {e}");
            std::process::exit(1);
        })
        .to_rgba8();

    let (w, h) = img.dimensions();
    let mut buf: Vec<[f32; 4]> = img
        .pixels()
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32])
        .collect();

    let mut rng = rand::thread_rng();
    let mut out = buf.clone();

    // 1. Wave displacement
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let yf = y as f32;
            let xf = x as f32;
            let disp_x = ((yf / 6.0).sin() * 3.0 + (xf / 9.0).sin() * 1.5) as i32;
            let disp_y = ((xf / 7.0).cos() * 3.0 + (yf / 10.0).cos() * 1.5) as i32;
            let si = idx(x + disp_x, y + disp_y, w, h);
            let di = (y as u32 * w + x as u32) as usize;
            out[di] = buf[si];
        }
    }

    buf.clone_from(&out);

    // 2. Chromatic aberration
    let shift: i32 = 2;
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let di = (y as u32 * w + x as u32) as usize;
            let r_i = idx(x + shift, y, w, h);
            let b_i = idx(x - shift, y, w, h);
            out[di][0] = buf[r_i][0];
            out[di][1] = buf[di][1];
            out[di][2] = buf[b_i][2];
            out[di][3] = buf[di][3];
        }
    }

    // 3. Glitch slices
    for _ in 0..6 {
        let y0 = rng.gen_range(0..h as i32);
        let bh = rng.gen_range(1..(h as i32 / 16).max(2));
        let y1 = (y0 + bh).min(h as i32);
        let shiftx = rng.gen_range(-(w as i32 / 8)..(w as i32 / 8));
        for y in y0..y1 {
            let mut row = vec![[0.0f32; 4]; w as usize];
            for x in 0..w as i32 {
                let src_x = (x - shiftx).rem_euclid(w as i32);
                let si = (y as u32 * w + src_x as u32) as usize;
                row[x as usize] = out[si];
            }
            for x in 0..w as usize {
                out[y as usize * w as usize + x] = row[x];
            }
        }
    }

    // 4. Pixel sorting by brightness on random rows
    for y in 0..h as usize {
        if rng.gen::<f32>() < 0.12 {
            let start = y * w as usize;
            let end = start + w as usize;
            let mut row: Vec<[f32; 4]> = out[start..end].to_vec();
            row.sort_by(|a, b| {
                let ga = a[0] * 0.299 + a[1] * 0.587 + a[2] * 0.114;
                let gb = b[0] * 0.299 + b[1] * 0.587 + b[2] * 0.114;
                ga.partial_cmp(&gb).unwrap_or(std::cmp::Ordering::Equal)
            });
            out[start..end].copy_from_slice(&row);
        }
    }

    // 5. Noise + contrast
    for p in &mut out {
        for c in 0..3 {
            let noise: f32 = rng.gen_range(-6.0..6.0);
            p[c] = (p[c] + noise - 128.0) * 1.15 + 128.0;
            p[c] = p[c].clamp(0.0, 255.0);
        }
        p[3] = p[3].clamp(0.0, 255.0);
    }

    let mut result = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = out[(y * w + x) as usize];
            result.put_pixel(x, y, Rgba([clamp_u8(p[0]), clamp_u8(p[1]), clamp_u8(p[2]), clamp_u8(p[3])]));
        }
    }

    let out_path = Path::new(output);
    result
        .save(out_path)
        .unwrap_or_else(|e| {
            eprintln!("Failed to save {output}: {e}");
            std::process::exit(1);
        });

    println!("done {}x{} -> {output}", w, h);
}
