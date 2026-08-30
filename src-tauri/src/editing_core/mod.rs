// Stub modules and early-development code still emit warnings;
// allow-list is kept intentional until each module is fully wired.
#![allow(dead_code, unused_imports, unused_variables)]

pub mod engine;
pub mod errors;
pub mod ipc;
pub mod types;

use std::sync::Mutex;

pub use engine::EditingEngine;
pub use ipc::*;
pub use types::*;

pub type EditingCoreState = Mutex<EditingEngine>;
