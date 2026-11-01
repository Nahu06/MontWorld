//! Construccion de la malla de un chunk.
//!
//! Solo se generan las caras que tocan aire. Un bloque rodeado por otros
//! no aporta geometria, que es la base para que el mundo sea barato de
//! dibujar. Cada vertice guarda posicion y color (el sombreado por cara
//! da sensacion de volumen sin iluminacion).

use super::block::Bloque;
use super::chunk::{Chunk, LADO};

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

/// Construye la malla de un chunk colocada en `origen` (coordenadas
/// globales del chunk) y consultando los vecinos con `obtener`.
///
/// `obtener` recibe coordenadas locales del chunk (pueden ser -1 o `LADO`),
/// lo que permite que dos chunks vecinos no dibujen la pared que comparten.
pub fn malla_con<F>(chunk: &Chunk, origen: [f32; 3], obtener: F) -> Vec<f32>
where
    F: Fn(i32, i32, i32) -> Bloque,
{
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
                    let vecino =
                        obtener(x + cara.normal[0], y + cara.normal[1], z + cara.normal[2]);
                    if vecino.es_aire() {
                        empujar_cara(&mut vertices, origen, x, y, z, cara, color);
                    }
                }
            }
        }
    }

    vertices
}

fn empujar_cara(
    salida: &mut Vec<f32>,
    origen: [f32; 3],
    x: i32,
    y: i32,
    z: i32,
    cara: &Cara,
    color: crate::base::math::Vec3,
) {
    let base = [
        origen[0] + x as f32,
        origen[1] + y as f32,
        origen[2] + z as f32,
    ];

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
    let mut chunk = Chunk::new((0, 0, 0));
    chunk.poner(x, y, z, Bloque::Piedra);
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOATS_POR_VERTICE: usize = 6;
    const VERTICES_POR_CARA: usize = 6;

    /// Malla de un chunk aislado: fuera de el todo es aire.
    fn malla(chunk: &Chunk) -> Vec<f32> {
        malla_con(chunk, [0.0, 0.0, 0.0], |x, y, z| chunk.bloque(x, y, z))
    }

    #[test]
    fn un_bloque_aislado_genera_seis_caras() {
        let chunk = chunk_con_bloque(8, 8, 8);
        let datos = malla(&chunk);
        let vertices = datos.len() / FLOATS_POR_VERTICE;
        assert_eq!(vertices, 6 * VERTICES_POR_CARA);
    }

    #[test]
    fn un_bloque_rodeado_no_genera_geometria() {
        let mut chunk = Chunk::new((0, 0, 0));
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
    fn un_bloque_pegado_al_borde_no_dibuja_la_cara_vecina() {
        // Un bloque en el borde con un vecino solido fuera del chunk no
        // debe generar la cara que comparten.
        let chunk = chunk_con_bloque(15, 8, 8);
        let datos = malla_con(&chunk, [0.0, 0.0, 0.0], |x, y, z| {
            if x == 16 && y == 8 && z == 8 {
                Bloque::Piedra
            } else {
                chunk.bloque(x, y, z)
            }
        });
        assert_eq!(datos.len() / FLOATS_POR_VERTICE, 5 * VERTICES_POR_CARA);
    }
}
