//! Construccion de la malla de un chunk.
//!
//! Solo se generan las caras que tocan aire. Un bloque rodeado por otros
//! no aporta geometria, que es la base para que el mundo sea barato de
//! dibujar. Cada vertice guarda posicion y color (el sombreado por cara
//! da sensacion de volumen sin iluminacion).

use super::chunk::{Chunk, LADO};
#[cfg(test)]
use super::block::Bloque;

struct Cara {
    /// Desplazamiento hasta el bloque vecino, para saber si hay aire.
    normal: [i32; 3],
    /// Las cuatro esquinas en orden, relativas al bloque.
    esquinas: [[f32; 3]; 4],
    /// Cuanto se oscurece esta cara respecto al color base.
    brillo: f32,
}

const CARAS: [Cara; 6] = [
    // +X
    Cara {
        normal: [1, 0, 0],
        esquinas: [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
        brillo: 0.84,
    },
    // -X
    Cara {
        normal: [-1, 0, 0],
        esquinas: [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ],
        brillo: 0.72,
    },
    // +Y (arriba)
    Cara {
        normal: [0, 1, 0],
        esquinas: [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        brillo: 1.0,
    },
    // -Y (abajo)
    Cara {
        normal: [0, -1, 0],
        esquinas: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
        brillo: 0.5,
    },
    // +Z
    Cara {
        normal: [0, 0, 1],
        esquinas: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        brillo: 0.92,
    },
    // -Z
    Cara {
        normal: [0, 0, -1],
        esquinas: [
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        brillo: 0.66,
    },
];

/// Devuelve los vertices (posicion xyz + color rgb) de las caras visibles.
pub fn malla(chunk: &Chunk) -> Vec<f32> {
    let mut vertices = Vec::with_capacity(4096);

    for y in 0..LADO as i32 {
        for z in 0..LADO as i32 {
            for x in 0..LADO as i32 {
                let bloque = chunk.bloque(x, y, z);
                if bloque.es_aire() {
                    continue;
                }

                let color = bloque.color();

                for cara in &CARAS {
                    let vecino = chunk.bloque(
                        x + cara.normal[0],
                        y + cara.normal[1],
                        z + cara.normal[2],
                    );
                    if vecino.es_aire() {
                        empujar_cara(&mut vertices, x, y, z, cara, color);
                    }
                }
            }
        }
    }

    vertices
}

fn empujar_cara(salida: &mut Vec<f32>, x: i32, y: i32, z: i32, cara: &Cara, color: crate::math::Vec3) {
    let base = [x as f32, y as f32, z as f32];

    let mut vertice = |esquina: [f32; 3]| {
        salida.extend_from_slice(&[
            base[0] + esquina[0],
            base[1] + esquina[1],
            base[2] + esquina[2],
            color.x * cara.brillo,
            color.y * cara.brillo,
            color.z * cara.brillo,
        ]);
    };

    vertice(cara.esquinas[0]);
    vertice(cara.esquinas[1]);
    vertice(cara.esquinas[2]);

    vertice(cara.esquinas[0]);
    vertice(cara.esquinas[2]);
    vertice(cara.esquinas[3]);
}

/// Coloca un bloque de prueba aislado para los tests.
#[cfg(test)]
fn chunk_con_bloque(x: i32, y: i32, z: i32) -> Chunk {
    let mut chunk = Chunk::new();
    chunk.poner(x, y, z, Bloque::Piedra);
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOATS_POR_VERTICE: usize = 6;
    const VERTICES_POR_CARA: usize = 6;

    #[test]
    fn un_bloque_aislado_genera_seis_caras() {
        let chunk = chunk_con_bloque(8, 8, 8);
        let datos = malla(&chunk);
        let vertices = datos.len() / FLOATS_POR_VERTICE;
        assert_eq!(vertices, 6 * VERTICES_POR_CARA);
    }

    #[test]
    fn un_bloque_rodeado_no_genera_geometria() {
        let mut chunk = Chunk::new();
        for y in 7..10 {
            for z in 7..10 {
                for x in 7..10 {
                    chunk.poner(x, y, z, Bloque::Piedra);
                }
            }
        }
        // Un cubo de 3x3x3: 54 caras exteriores y ninguna interior.
        assert_eq!(malla(&chunk).len() / FLOATS_POR_VERTICE, 54 * VERTICES_POR_CARA);
    }

    #[test]
    fn el_borde_del_chunk_dibuja_sus_caras() {
        let chunk = chunk_con_bloque(0, 0, 0);
        let datos = malla(&chunk);
        assert_eq!(datos.len() / FLOATS_POR_VERTICE, 6 * VERTICES_POR_CARA);
    }

    #[test]
    fn el_terreno_tiene_bloques_y_aire_arriba() {
        let mut chunk = Chunk::new();
        chunk.generar_terreno();
        let mut solidos = 0;
        for y in 0..LADO as i32 {
            for z in 0..LADO as i32 {
                for x in 0..LADO as i32 {
                    if !chunk.bloque(x, y, z).es_aire() {
                        solidos += 1;
                    }
                }
            }
        }
        assert!(solidos > 0);
        assert_eq!(chunk.bloque(8, 15, 8), Bloque::Aire);
    }
}
