use std::cell::Cell;

use mupdf::{Document as MuDocument, TextPageFlags};

use super::engine::{OpenedDocument, PdfEngine};
use super::errors::DocumentCoreError;
use super::types::{BBox, TextExtractionRequest, TextExtractionResponse, TextSpan};

pub struct TextExtractionPipeline {
    pub total_extractions: Cell<usize>,
}

impl Default for TextExtractionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl TextExtractionPipeline {
    pub fn new() -> Self {
        Self {
            total_extractions: Cell::new(0),
        }
    }

    pub fn extract(
        &self,
        engine: &dyn PdfEngine,
        doc: &OpenedDocument,
        request: &TextExtractionRequest,
    ) -> Result<TextExtractionResponse, DocumentCoreError> {
        let response = engine.extract_text(doc, request)?;
        self.total_extractions.set(self.total_extractions.get() + 1);
        Ok(response)
    }

    pub fn extract_from_parsed(
        &self,
        parsed_doc: &MuDocument,
        doc: &OpenedDocument,
        request: &TextExtractionRequest,
    ) -> Result<TextExtractionResponse, DocumentCoreError> {
        if request.page_index >= doc.pages.len() {
            return Err(DocumentCoreError::PageOutOfRange {
                requested: request.page_index,
                total: doc.pages.len(),
            });
        }

        let page = parsed_doc
            .load_page(
                i32::try_from(request.page_index)
                    .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?,
            )
            .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
        let text_page = page
            .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
            .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
        let full_text = text_page
            .to_text()
            .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
        let spans = if full_text.trim().is_empty() {
            Vec::new()
        } else {
            vec![TextSpan {
                content: full_text.clone(),
                bbox: BBox {
                    x: 0.0,
                    y: 0.0,
                    width: doc
                        .pages
                        .get(request.page_index)
                        .map(|page| page.width_points)
                        .unwrap_or(0.0),
                    height: doc
                        .pages
                        .get(request.page_index)
                        .map(|page| page.height_points)
                        .unwrap_or(0.0),
                },
                font_name: None,
            }]
        };

        self.total_extractions.set(self.total_extractions.get() + 1);
        Ok(TextExtractionResponse {
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            full_text,
            spans,
        })
    }

    pub fn extract_all_from_parsed(
        &self,
        parsed_doc: &MuDocument,
        doc: &OpenedDocument,
    ) -> Result<Vec<String>, DocumentCoreError> {
        let mut result = Vec::with_capacity(doc.pages.len());
        for page_index in 0..doc.pages.len() {
            let page = parsed_doc
                .load_page(
                    i32::try_from(page_index)
                        .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?,
                )
                .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
            let text_page = page
                .to_text_page(TextPageFlags::PRESERVE_WHITESPACE)
                .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
            let text = text_page
                .to_text()
                .map_err(|e| DocumentCoreError::TextExtractionError(e.to_string()))?;
            result.push(text);
        }
        self.total_extractions
            .set(self.total_extractions.get() + doc.pages.len());
        Ok(result)
    }
}
