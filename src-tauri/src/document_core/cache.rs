use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderCacheKey {
    pub session_id: String,
    pub page_index: usize,
    pub rotation: i32,
    pub zoom_bucket: u32,
    pub render_format: String,
    pub document_revision: u64,
    pub viewport_key: String,
}

#[derive(Debug, Clone)]
pub struct CachedRender {
    pub width_px: u32,
    pub height_px: u32,
    pub pixels_rgba: Vec<u8>,
    /// Page width in PDF points (zoom-independent). Kept in the cache so
    /// cache-hit render responses still carry authoritative dimensions for
    /// the canvas hit-test overlay and inline text editor.
    pub width_pts: f32,
    pub height_pts: f32,
}

#[derive(Debug)]
pub struct PageRenderCache {
    max_entries: usize,
    max_bytes: usize,
    bytes: usize,
    map: HashMap<RenderCacheKey, CachedRender>,
    order: VecDeque<RenderCacheKey>,
}

impl PageRenderCache {
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            max_entries,
            max_bytes,
            bytes: 0,
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &RenderCacheKey) -> Option<CachedRender> {
        let value = self.map.get(key).cloned();
        if value.is_some() {
            self.touch(key);
        }
        value
    }

    pub fn insert(&mut self, key: RenderCacheKey, value: CachedRender) {
        let value_len = value.pixels_rgba.len();

        if let Some(previous) = self.map.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.pixels_rgba.len());
            self.order.retain(|item| item != &key);
        }

        self.bytes += value_len;
        self.order.push_back(key.clone());
        self.map.insert(key, value);

        self.evict_to_fit();
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
        self.bytes = 0;
    }

    pub fn invalidate_page(&mut self, session_id: &str, page_index: usize) {
        let keys: Vec<RenderCacheKey> = self
            .map
            .keys()
            .filter(|key| key.session_id == session_id && key.page_index == page_index)
            .cloned()
            .collect();
        for key in keys {
            if let Some(value) = self.map.remove(&key) {
                self.bytes = self.bytes.saturating_sub(value.pixels_rgba.len());
            }
            self.order.retain(|item| item != &key);
        }
    }

    fn touch(&mut self, key: &RenderCacheKey) {
        self.order.retain(|item| item != key);
        self.order.push_back(key.clone());
    }

    fn evict_to_fit(&mut self) {
        while self.map.len() > self.max_entries || self.bytes > self.max_bytes {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            if let Some(value) = self.map.remove(&key) {
                self.bytes = self.bytes.saturating_sub(value.pixels_rgba.len());
            }
        }
    }
}
