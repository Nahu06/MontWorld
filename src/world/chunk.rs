//! Chunk: una porcion cubica del mundo.
//!
//! Se guarda como una lista plana de bloques de `LADO^3` posiciones. Las
//! coordenadas internas van de 0 a `LADO - 1`.

use super::block::Bloque;

/// Bloques por lado del chunk. Al ser potencia de dos, las cuentas son
/// baratas y el chunk ocupa 4096 bloques (4 KiB).
pub const LADO: usize = 16;
const LADO_I: i32 = LADO as i32;
const AREA: usize = LADO * LADO;
const VOLUMEN: usize = AREA * LADO;

pub struct Chunk {
    bloques: Vec<Bloque>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            bloques: vec![Bloque::Aire; VOLUMEN],
        }
    }

    fn indice(x: usize, y: usize, z: usize) -> usize {
        x + z * LADO + y * AREA
    }

    /// Lee un bloque. Fuera del chunk se considera aire, de modo que las
    /// caras del borde se dibujan.
    pub fn bloque(&self, x: i32, y: i32, z: i32) -> Bloque {
        if x < 0 || y < 0 || z < 0 || x >= LADO_I || y >= LADO_I || z >= LADO_I {
            return Bloque::Aire;
        }
        self.bloques[Self::indice(x as usize, y as usize, z as usize)]
    }

    pub fn poner(&mut self, x: i32, y: i32, z: i32, bloque: Bloque) {
        if x < 0 || y < 0 || z < 0 || x >= LADO_I || y >= LADO_I || z >= LADO_I {
            return;
        }
        self.bloques[Self::indice(x as usize, y as usize, z as usize)] = bloque;
    }

    /// Rellena el chunk con un terreno de prueba: piedra abajo, tierra en
    /// medio y hierba en la superficie. La altura sale de una suma de
    /// senos, suficiente para que el relieve no sea plano.
    pub fn generar_terreno(&mut self) {
        for z in 0..LADO_I {
            for x in 0..LADO_I {
                let altura = altura_terreno(x, z);

                for y in 0..altura {
                    let bloque = if y == altura - 1 {
                        // Las zonas bajas quedan como playa.
                        if altura <= 5 { Bloque::Arena } else { Bloque::Hierba }
                    } else if y >= altura - 3 {
                        Bloque::Tierra
                    } else {
                        Bloque::Piedra
                    };
                    self.poner(x, y, z, bloque);
                }
            }
        }
    }
}

/// Altura de la superficie en coordenadas del chunk. Va de 4 a 12 para
/// que quepa de sobra en los 16 bloques de alto.
fn altura_terreno(x: i32, z: i32) -> i32 {
    let x = x as f32;
    let z = z as f32;
    let relieve = (x * 0.42).sin() * 1.8 + (z * 0.37).cos() * 1.5 + ((x + z) * 0.19).sin() * 1.2;
    let altura = 8.0 + relieve;
    altura.round().clamp(4.0, 12.0) as i32
}
