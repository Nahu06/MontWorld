//! El mundo: bloques y chunk.
//!
//! De momento el mundo es un unico chunk de prueba. La idea es que este
//! modulo crezca con varios chunks, generacion y guardado.

pub mod block;
pub mod chunk;
pub mod meshing;

pub use chunk::Chunk;
