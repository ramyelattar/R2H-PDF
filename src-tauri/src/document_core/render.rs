use std::cell::{Cell, RefCell};
use std::time::Instant;

use mupdf::{Colorspace, Document as MuDocument, Matrix};

use super::cache::{CachedRender, PageRenderCache, RenderCacheKey};
use super::engine::{OpenedDocument, PdfEngine};
use super::errors::DocumentCoreError;
use super::types::{RenderRequest, RenderResponse};

pub struct RenderPipeline {
    cache: RefCell<PageRenderCache>,
    pub total_renders: Cell<usize>,
    pub cache_hits: Cell<usize>,
    pub cache_misses: Cell<usize>,
}

impl RenderPipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(PageRenderCache::new(256, 256 * 1024 * 1024)),
            total_renders: Cell::new(0),
            cache_hits: Cell::new(0),
            cache_misses: Cell::new(0),
        }
    }

    pub fn render(
        &self,
        engine: &dyn PdfEngine,
        doc: &OpenedDocument,
        request: &RenderRequest,
    ) -> Result<RenderResponse, DocumentCoreError> {
        let key = Self::build_key(request, 0, 0, "rgba");

        if let Some(cached) = self.cache.borrow_mut().get(&key) {
            self.cache_hits.set(self.cache_hits.get() + 1);
            return Ok(RenderResponse {
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                width_px: cached.width_px,
                height_px: cached.height_px,
                width: cached.width_px,
                height: cached.height_px,
                zoom: request.zoom,
                cache_hit: true,
                pixels_rgba: cached.pixels_rgba,
                render_time_ms: 0,
                width_pts: cached.width_pts,
                height_pts: cached.height_pts,
            });
        }

        let mut rendered = engine.render_page(doc, request)?;
        rendered.cache_hit = false;
        self.cache_misses.set(self.cache_misses.get() + 1);

        self.cache.borrow_mut().insert(
            key,
            CachedRender {
                width_px: rendered.width_px,
                height_px: rendered.height_px,
                pixels_rgba: rendered.pixels_rgba.clone(),
                width_pts: rendered.width_pts,
                height_pts: rendered.height_pts,
            },
        );

        self.total_renders.set(self.total_renders.get() + 1);

        Ok(rendered)
    }

    /// Render using a pre-parsed MuDocument handle, avoiding re-parse overhead.
    /// The caller is responsible for providing a valid parsed document that
    /// corresponds to the current document bytes.
    pub fn render_from_parsed(
        &self,
        parsed_doc: &MuDocument,
        doc: &OpenedDocument,
        request: &RenderRequest,
        document_revision: u64,
        rotation: i32,
    ) -> Result<RenderResponse, DocumentCoreError> {
        let key = Self::build_key(request, document_revision, rotation, "rgba");

        // Check pixel cache first.
        if let Some(cached) = self.cache.borrow_mut().get(&key) {
            self.cache_hits.set(self.cache_hits.get() + 1);
            return Ok(RenderResponse {
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                width_px: cached.width_px,
                height_px: cached.height_px,
                width: cached.width_px,
                height: cached.height_px,
                zoom: request.zoom,
                cache_hit: true,
                pixels_rgba: cached.pixels_rgba,
                render_time_ms: 0,
                width_pts: cached.width_pts,
                height_pts: cached.height_pts,
            });
        }

        // Validate page index.
        if request.page_index >= doc.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: request.page_index,
                total: doc.pages.len(),
            });
        }

        let start = Instant::now();
        self.cache_misses.set(self.cache_misses.get() + 1);
        let page = parsed_doc
            .load_page(i32::try_from(request.page_index)
                .map_err(|e| DocumentCoreError::RenderError(e.to_string()))?)
            .map_err(|e| DocumentCoreError::RenderError(e.to_string()))?;

        let page_bounds = page
            .bounds()
            .map_err(|e| DocumentCoreError::RenderError(e.to_string()))?;
        let width_pts = page_bounds.width().abs();
        let height_pts = page_bounds.height().abs();

        let scale = (request.zoom.max(0.25) * request.device_pixel_ratio.max(1.0)).max(0.1);
        let matrix = Matrix::new_scale(scale, scale);
        let pixmap = page
            .to_pixmap(&matrix, &Colorspace::device_rgb(), true, true)
            .map_err(|e| DocumentCoreError::RenderError(e.to_string()))?;

        let width_px = pixmap.width();
        let height_px = pixmap.height();
        let pixels_rgba = pixmap.samples().to_vec();
        let render_time_ms = start.elapsed().as_millis();
        let expected_len = (width_px as usize) * (height_px as usize) * 4;

        if width_px == 0 || height_px == 0 {
            return Err(DocumentCoreError::RenderError(format!(
                "cached_doc renderer produced zero-size pixmap for session={} page={} ({}x{})",
                request.session_id, request.page_index, width_px, height_px,
            )));
        }
        if pixels_rgba.is_empty() {
            return Err(DocumentCoreError::RenderError(format!(
                "cached_doc renderer produced an empty pixel buffer for session={} page={}",
                request.session_id, request.page_index,
            )));
        }
        if pixels_rgba.len() != expected_len {
            return Err(DocumentCoreError::RenderError(format!(
                "cached_doc renderer pixel-buffer length mismatch for session={} page={}: got {} bytes, expected {}",
                request.session_id, request.page_index, pixels_rgba.len(), expected_len,
            )));
        }

        if cfg!(debug_assertions) {
            eprintln!(
                "[render_pipeline] cached_doc render session={} page={} zoom={:.3} size={}x{} bytes={} time={}ms",
                request.session_id, request.page_index, request.zoom,
                width_px, height_px, pixels_rgba.len(), render_time_ms
            );
        }

        let rendered = RenderResponse {
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            width_px,
            height_px,
            width: width_px,
            height: height_px,
            zoom: request.zoom,
            cache_hit: false,
            pixels_rgba: pixels_rgba.clone(),
            render_time_ms,
            width_pts,
            height_pts,
        };

        self.cache.borrow_mut().insert(
            key,
            CachedRender {
                width_px,
                height_px,
                pixels_rgba,
                width_pts,
                height_pts,
            },
        );

        self.total_renders.set(self.total_renders.get() + 1);

        Ok(rendered)
    }

    pub fn prefetch_hints(&self, page_index: usize, page_count: usize) -> Vec<usize> {
        let mut hints = Vec::new();
        if page_index > 0 {
            hints.push(page_index - 1);
        }
        if page_index + 1 < page_count {
            hints.push(page_index + 1);
        }
        if page_index + 2 < page_count {
            hints.push(page_index + 2);
        }
        hints
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.borrow().len(), self.cache.borrow().bytes())
    }

    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }

    pub fn invalidate_page_cache(&self, session_id: &str, page_index: usize) {
        self.cache.borrow_mut().invalidate_page(session_id, page_index);
    }

    fn build_key(
        request: &RenderRequest,
        document_revision: u64,
        rotation: i32,
        render_format: &str,
    ) -> RenderCacheKey {
        let zoom_bucket = (request.zoom * 100.0).round() as u32;
        let viewport_key = format!(
            "{:.0}:{:.0}:{:.0}:{:.0}:{}",
            request.viewport.x,
            request.viewport.y,
            request.viewport.width,
            request.viewport.height,
            (request.device_pixel_ratio * 100.0).round() as u32
        );

        RenderCacheKey {
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            rotation,
            zoom_bucket,
            render_format: render_format.to_string(),
            document_revision,
            viewport_key,
        }
    }
}
