//! El mundo: bloques, chunks y raycast.
//!
//! El punto de entrada habitual es `Mundo`, que agrupa los chunks cargados
//! y resuelve las coordenadas globales.

pub mod block;
pub mod chunk;
pub mod generacion;
pub mod meshing;
pub mod mundo;
pub mod raycast;

pub use mundo::Mundo;
