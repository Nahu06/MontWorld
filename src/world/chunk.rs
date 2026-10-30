//! Chunk: una porcion cubica del mundo.
//!
//! Se guarda como una lista plana de bloques de `LADO^3` posiciones. Las
//! coordenadas internas van de 0 a `LADO - 1`.
//!
//! La generacion usa coordenadas globales, de modo que dos chunks vecinos
//! encajan sin costuras.

use super::block::Bloque;

/// Bloques por lado del chunk. Al ser potencia de dos, las cuentas son
/// baratas y el chunk ocupa 4096 bloques (4 KiB).
pub const LADO: usize = 16;
pub const LADO_I: i32 = LADO as i32;
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

    /// Lee un bloque. Fuera del chunk se considera aire.
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

    /// Genera el terreno del chunk situado en la rejilla `(cx, cz)`.
    pub fn generar(cx: i32, cz: i32) -> Self {
        let mut chunk = Self::new();

        for z in 0..LADO_I {
            for x in 0..LADO_I {
                let altura = altura_terreno(cx * LADO_I + x, cz * LADO_I + z);

                for y in 0..altura {
                    let bloque = if y == altura - 1 {
                        // Las zonas bajas quedan como playa.
                        if altura <= 5 { Bloque::Arena } else { Bloque::Hierba }
                    } else if y >= altura - 3 {
                        Bloque::Tierra
                    } else {
                        Bloque::Piedra
                    };
                    chunk.poner(x, y, z, bloque);
                }
            }
        }

        chunk
    }
}

/// Altura de la superficie en coordenadas globales. Va de 3 a 13 para que
/// quepa de sobra en los 16 bloques de alto.
fn altura_terreno(x: i32, z: i32) -> i32 {
    let x = x as f32;
    let z = z as f32;
    let altura = 8.0 + ruido_suave(x, z, 26.0) * 3.0 + ruido_suave(x, z, 9.0) * 1.2;
    altura.round().clamp(3.0, 13.0) as i32
}

/// Ruido de valor con interpolacion suave. Es determinista y solo depende
/// de las coordenadas, asi que los chunks vecinos coinciden en el borde.
fn ruido_suave(x: f32, z: f32, escala: f32) -> f32 {
    let fx = x / escala;
    let fz = z / escala;

    let x0 = fx.floor();
    let z0 = fz.floor();
    let tx = suavizar(fx - x0);
    let tz = suavizar(fz - z0);

    let (xi, zi) = (x0 as i32, z0 as i32);
    let a = ruido_entero(xi, zi);
    let b = ruido_entero(xi + 1, zi);
    let c = ruido_entero(xi, zi + 1);
    let d = ruido_entero(xi + 1, zi + 1);

    let arriba = a + (b - a) * tx;
    let abajo = c + (d - c) * tx;
    arriba + (abajo - arriba) * tz
}

/// Curva de suavizado: 3t² - 2t³.
fn suavizar(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Valor pseudoaleatorio en [-1, 1] para una celda de la rejilla de ruido.
fn ruido_entero(x: i32, z: i32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393).wrapping_add(z.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    let unidad = ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0;
    unidad * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_terreno_tiene_bloques_y_aire_arriba() {
        let chunk = Chunk::generar(0, 0);
        let mut solidos = 0;
        for y in 0..LADO_I {
            for z in 0..LADO_I {
                for x in 0..LADO_I {
                    if !chunk.bloque(x, y, z).es_aire() {
                        solidos += 1;
                    }
                }
            }
        }
        assert!(solidos > 0);
        assert_eq!(chunk.bloque(8, 15, 8), Bloque::Aire);
    }

    #[test]
    fn la_generacion_usa_coordenadas_globales() {
        // El chunk (1, 0) empieza en la coordenada global x = 16: su
        // columna local 0 debe coincidir con la altura calculada alli.
        let chunk = Chunk::generar(1, 0);
        for z in 0..LADO_I {
            let altura = altura_terreno(LADO_I, z);
            assert!(!chunk.bloque(0, altura - 1, z).es_aire());
            if altura < LADO_I {
                assert!(chunk.bloque(0, altura, z).es_aire());
            }
        }
    }

    #[test]
    fn la_generacion_es_determinista() {
        let a = Chunk::generar(3, -2);
        let b = Chunk::generar(3, -2);
        for y in 0..LADO_I {
            for z in 0..LADO_I {
                for x in 0..LADO_I {
                    assert_eq!(a.bloque(x, y, z), b.bloque(x, y, z));
                }
            }
        }
    }
}
