//! GUI smoke test for CI: run the real app on a real window system (Xvfb / headless
//! Weston), wait for frames to be presented, grab the framebuffer, check it is not blank,
//! save it and exit. Enabled by `ARCANA_SMOKE_SCREENSHOT=<file.ppm>`.
//!
//! Exit code: 0 = rendered, 2 = blank frame, 3 = could not write the file,
//! 4 = no screenshot within the frame budget.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

/// Frames to wait before grabbing (layout, fonts, first async loads).
const WARMUP_FRAMES: u32 = 30;
/// Give up after this many frames without a screenshot event.
const MAX_FRAMES: u32 = 600;

pub struct Smoke {
    out: PathBuf,
    exit: Arc<AtomicI32>,
    frames: u32,
    requested: bool,
}

impl Smoke {
    pub fn new(out: PathBuf, exit: Arc<AtomicI32>) -> Self {
        Self {
            out,
            exit,
            frames: 0,
            requested: false,
        }
    }

    pub fn frame(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        ctx.request_repaint();
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let code = self.finish(&image);
            self.exit.store(code, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames >= WARMUP_FRAMES && !self.requested {
            self.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if self.frames >= MAX_FRAMES {
            eprintln!("smoke: no screenshot after {MAX_FRAMES} frames");
            self.exit.store(4, Ordering::SeqCst);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn finish(&self, image: &egui::ColorImage) -> i32 {
        let distinct = distinct_colors(image);
        eprintln!(
            "smoke: {}x{} frame, {distinct} distinct colours",
            image.size[0], image.size[1]
        );
        if let Err(e) = std::fs::write(&self.out, ppm(image)) {
            eprintln!("smoke: cannot write {}: {e}", self.out.display());
            return 3;
        }
        if is_blank(image) { 2 } else { 0 }
    }
}

/// Number of distinct RGB values (capped; enough to tell a UI from a clear colour).
pub fn distinct_colors(image: &egui::ColorImage) -> usize {
    let mut seen = std::collections::HashSet::new();
    for p in &image.pixels {
        seen.insert([p.r(), p.g(), p.b()]);
        if seen.len() > 1000 {
            break;
        }
    }
    seen.len()
}

/// A frame of one or two colours is a clear colour / failed render, not the shell.
pub fn is_blank(image: &egui::ColorImage) -> bool {
    image.pixels.is_empty() || distinct_colors(image) < 16
}

/// Binary PPM (P6): no image-encoder dependency; `convert x.ppm x.png` if needed.
pub fn ppm(image: &egui::ColorImage) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", image.size[0], image.size[1]).into_bytes();
    for p in &image.pixels {
        out.extend_from_slice(&[p.r(), p.g(), p.b()]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_and_non_blank_frames() {
        let flat = egui::ColorImage::new([4, 4], vec![egui::Color32::WHITE; 16]);
        assert!(is_blank(&flat));
        let px: Vec<egui::Color32> = (0..64u8)
            .map(|i| egui::Color32::from_rgb(i * 4, 255 - i, i))
            .collect();
        let busy = egui::ColorImage::new([8, 8], px);
        assert!(!is_blank(&busy));
        assert!(ppm(&busy).starts_with(b"P6\n8 8\n255\n"));
        assert_eq!(ppm(&busy).len(), 11 + 64 * 3);
    }
}
