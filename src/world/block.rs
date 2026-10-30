//! Tipos de bloque del mundo.
//!
//! Se guardan como un byte para que un chunk ocupe poco memoria. El valor
//! 0 siempre es aire.

use crate::base::math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Bloque {
    Aire = 0,
    Tierra = 1,
    Piedra = 2,
    Hierba = 3,
    Arena = 4,
}

impl Bloque {
    /// Convierte el byte guardado en disco al tipo de bloque.
    pub fn desde_byte(valor: u8) -> Self {
        match valor {
            1 => Self::Tierra,
            2 => Self::Piedra,
            3 => Self::Hierba,
            4 => Self::Arena,
            _ => Self::Aire,
        }
    }

    pub fn es_aire(self) -> bool {
        self == Self::Aire
    }

    /// Color base del bloque, sin sombreado por cara.
    pub fn color(self) -> Vec3 {
        match self {
            Self::Aire => Vec3::new(0.0, 0.0, 0.0),
            Self::Tierra => Vec3::new(0.47, 0.33, 0.21),
            Self::Piedra => Vec3::new(0.55, 0.55, 0.58),
            Self::Hierba => Vec3::new(0.34, 0.62, 0.26),
            Self::Arena => Vec3::new(0.85, 0.79, 0.55),
        }
    }
}
