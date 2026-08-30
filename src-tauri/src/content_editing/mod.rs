//! Native PDF content editing engine.
//! Provides true content stream manipulation for text and image objects.

pub mod analysis;
pub mod stream_parser;
pub mod text_edit;
pub mod text_block;
pub mod font_registry;
pub mod tounicode;
pub mod encoding_map;
pub mod image_edit;
pub mod background_sampling;
pub mod find_replace;
pub mod history;
pub mod types;
pub mod ipc;

#[cfg(test)]
mod round_trip_tests;
