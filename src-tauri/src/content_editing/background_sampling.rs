//! Rendered-pixel background sampling for safe visual text covers.

use crate::document_core::DocumentCoreState;
use mupdf::{Colorspace, Matrix};

#[derive(Debug, Clone, PartialEq)]
pub struct BackgroundSample {
    pub rgb: [f32; 3],
    pub confidence: f32,
    pub used_fallback: bool,
    pub warning: Option<String>,
}

impl BackgroundSample {
    pub fn white_fallback() -> Self {
        Self {
            rgb: [1.0, 1.0, 1.0],
            confidence: 0.0,
            used_fallback: true,
            warning: Some(
                "Background could not be sampled reliably; white cover used.".to_string(),
            ),
        }
    }
}

pub fn sample_page_background(
    doc_state: &DocumentCoreState,
    session_id: &str,
    page_index: usize,
    bbox: [f32; 4],
) -> BackgroundSample {
    let bytes = match doc_state.store.get_session_arc_pub(session_id) {
        Ok(arc) => match arc.lock() {
            Ok(session) => session.document.bytes.clone(),
            Err(_) => return BackgroundSample::white_fallback(),
        },
        Err(_) => return BackgroundSample::white_fallback(),
    };
    let doc = match mupdf::Document::from_bytes(&bytes, "pdf") {
        Ok(d) => d,
        Err(_) => return BackgroundSample::white_fallback(),
    };
    let page = match doc.load_page(page_index as i32) {
        Ok(p) => p,
        Err(_) => return BackgroundSample::white_fallback(),
    };
    let bounds = match page.bounds() {
        Ok(b) => b,
        Err(_) => return BackgroundSample::white_fallback(),
    };
    let page_w = bounds.width().abs();
    let page_h = bounds.height().abs();
    if page_w <= 0.0 || page_h <= 0.0 {
        return BackgroundSample::white_fallback();
    }
    let scale = 1.5f32;
    let pixmap = match page.to_pixmap(
        &Matrix::new_scale(scale, scale),
        &Colorspace::device_rgb(),
        true,
        true,
    ) {
        Ok(p) => p,
        Err(_) => return BackgroundSample::white_fallback(),
    };
    sample_background_from_rgba(
        pixmap.samples(),
        pixmap.width() as usize,
        pixmap.height() as usize,
        page_w,
        page_h,
        bbox,
        scale,
    )
}

pub fn sample_background_from_rgba(
    pixels: &[u8],
    width_px: usize,
    height_px: usize,
    page_width_pts: f32,
    page_height_pts: f32,
    bbox: [f32; 4],
    scale: f32,
) -> BackgroundSample {
    if pixels.len() < width_px.saturating_mul(height_px).saturating_mul(4)
        || width_px == 0
        || height_px == 0
        || page_width_pts <= 0.0
        || page_height_pts <= 0.0
        || bbox.iter().any(|v| !v.is_finite())
        || bbox[2] <= bbox[0]
        || bbox[3] <= bbox[1]
    {
        return BackgroundSample::white_fallback();
    }

    let mut samples: Vec<[u8; 3]> = Vec::new();
    let pad = 3.0f32.max(2.0 / scale.max(0.1));
    let step = (2.0 / scale.max(0.1)).max(0.75);
    let x0 = bbox[0].max(0.0);
    let y0 = bbox[1].max(0.0);
    let x1 = bbox[2].min(page_width_pts);
    let y1 = bbox[3].min(page_height_pts);

    let mut push_pdf_point = |x: f32, y: f32| {
        if x < 0.0 || y < 0.0 || x > page_width_pts || y > page_height_pts {
            return;
        }
        let px = (x * scale).round() as isize;
        let py = ((page_height_pts - y) * scale).round() as isize;
        if px < 0 || py < 0 || px >= width_px as isize || py >= height_px as isize {
            return;
        }
        let idx = ((py as usize) * width_px + (px as usize)) * 4;
        if idx + 2 < pixels.len() {
            samples.push([pixels[idx], pixels[idx + 1], pixels[idx + 2]]);
        }
    };

    let mut x = x0;
    while x <= x1 {
        push_pdf_point(x, (y1 + pad).min(page_height_pts));
        push_pdf_point(x, (y0 - pad).max(0.0));
        x += step;
    }
    let mut y = y0;
    while y <= y1 {
        push_pdf_point((x0 - pad).max(0.0), y);
        push_pdf_point((x1 + pad).min(page_width_pts), y);
        y += step;
    }

    dominant_rgb(&samples).unwrap_or_else(BackgroundSample::white_fallback)
}

fn dominant_rgb(samples: &[[u8; 3]]) -> Option<BackgroundSample> {
    if samples.len() < 8 {
        return None;
    }
    let mut rs: Vec<u8> = samples.iter().map(|p| p[0]).collect();
    let mut gs: Vec<u8> = samples.iter().map(|p| p[1]).collect();
    let mut bs: Vec<u8> = samples.iter().map(|p| p[2]).collect();
    rs.sort_unstable();
    gs.sort_unstable();
    bs.sort_unstable();
    let mid = samples.len() / 2;
    let med = [rs[mid], gs[mid], bs[mid]];
    let close = samples
        .iter()
        .filter(|p| color_distance_sq(**p, med) <= 18 * 18)
        .count();
    let confidence = close as f32 / samples.len() as f32;
    if confidence < 0.80 {
        return None;
    }
    Some(BackgroundSample {
        rgb: [
            med[0] as f32 / 255.0,
            med[1] as f32 / 255.0,
            med[2] as f32 / 255.0,
        ],
        confidence,
        used_fallback: false,
        warning: None,
    })
}

fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize, rgb: [u8; 3]) -> Vec<u8> {
        let mut out = Vec::with_capacity(width * height * 4);
        for _ in 0..(width * height) {
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        out
    }

    #[test]
    fn white_background_samples_white() {
        let pixels = solid(200, 200, [255, 255, 255]);
        let sample = sample_background_from_rgba(
            &pixels,
            200,
            200,
            100.0,
            100.0,
            [40.0, 20.0, 60.0, 40.0],
            2.0,
        );
        assert!(!sample.used_fallback);
        assert!(sample.rgb.iter().all(|v| (*v - 1.0).abs() < 0.01));
    }

    #[test]
    fn gray_background_samples_gray() {
        let pixels = solid(200, 200, [188, 188, 188]);
        let sample = sample_background_from_rgba(
            &pixels,
            200,
            200,
            100.0,
            100.0,
            [20.0, 20.0, 60.0, 40.0],
            2.0,
        );
        assert!(!sample.used_fallback);
        assert!((sample.rgb[0] - 188.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn colored_background_samples_color() {
        let pixels = solid(200, 200, [80, 150, 210]);
        let sample = sample_background_from_rgba(
            &pixels,
            200,
            200,
            100.0,
            100.0,
            [20.0, 20.0, 60.0, 40.0],
            2.0,
        );
        assert!(!sample.used_fallback);
        assert!((sample.rgb[2] - 210.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn low_confidence_falls_back_with_warning() {
        let mut pixels = Vec::new();
        for _y in 0..200 {
            for x in 0..200 {
                let rgb = if x < 100 { [0, 0, 0] } else { [255, 255, 255] };
                pixels.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        let sample = sample_background_from_rgba(
            &pixels,
            200,
            200,
            100.0,
            100.0,
            [20.0, 20.0, 60.0, 40.0],
            2.0,
        );
        assert!(sample.used_fallback);
        assert_eq!(
            sample.warning.as_deref(),
            Some("Background could not be sampled reliably; white cover used.")
        );
    }
}
