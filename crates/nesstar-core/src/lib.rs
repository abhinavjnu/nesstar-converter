//! Shared parser, decoder, validation, and output primitives.

pub mod ddi;
pub mod decode;
pub mod error;
pub mod formats;
pub mod layout;
pub mod model;
pub mod pipeline;
pub mod report;
pub mod source;
pub mod validation;

pub use error::NesstarError;
