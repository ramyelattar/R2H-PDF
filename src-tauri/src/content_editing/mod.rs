//! Native PDF content editing engine.
//! Provides true content stream manipulation for text and image objects.

pub mod analysis;
pub mod background_sampling;
pub mod encoding_map;
pub mod find_replace;
pub mod font_registry;
pub mod history;
pub mod image_edit;
pub mod ipc;
pub mod stream_parser;
pub mod text_block;
pub mod text_edit;
pub mod tounicode;
pub mod types;

#[cfg(test)]
mod round_trip_tests;
