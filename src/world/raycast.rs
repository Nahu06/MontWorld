//! Raycast sobre vóxeles.
//!
//! Se recorre la rejilla de bloques con el algoritmo de Amanatides y Woo
//! (DDA 3D): avanza de celda en celda y se detiene en el primer bloque
//! solido. Es la forma barata de saber a que bloque apunta la camara, sin
//! necesidad de mallas de colision.

use crate::base::math::Vec3;

use super::block::Bloque;

/// Resultado de un impacto: el bloque golpeado y la cara por la que entro
/// el rayo (normal entera, por ejemplo [0, 1, 0] si vino de arriba).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Impacto {
    pub bloque: [i32; 3],
    pub normal: [i32; 3],
}

pub fn lanzar<F>(origen: Vec3, direccion: Vec3, alcance: f32, bloque: F) -> Option<Impacto>
where
    F: Fn(i32, i32, i32) -> Bloque,
{
    let dir = direccion.normalize();
    if dir.length() == 0.0 {
        return None;
    }

    let mut x = origen.x.floor() as i32;
    let mut y = origen.y.floor() as i32;
    let mut z = origen.z.floor() as i32;

    let paso = [
        if dir.x > 0.0 { 1 } else if dir.x < 0.0 { -1 } else { 0 },
        if dir.y > 0.0 { 1 } else if dir.y < 0.0 { -1 } else { 0 },
        if dir.z > 0.0 { 1 } else if dir.z < 0.0 { -1 } else { 0 },
    ];

    // Distancia entre cruces de celda en cada eje.
    let delta = [
        if paso[0] != 0 { (1.0f32 / dir.x).abs() } else { f32::INFINITY },
        if paso[1] != 0 { (1.0f32 / dir.y).abs() } else { f32::INFINITY },
        if paso[2] != 0 { (1.0f32 / dir.z).abs() } else { f32::INFINITY },
    ];

    // Distancia hasta el primer cruce de celda.
    let mut siguiente = [
        if paso[0] > 0 {
            (x as f32 + 1.0 - origen.x) / dir.x
        } else if paso[0] < 0 {
            (x as f32 - origen.x) / dir.x
        } else {
            f32::INFINITY
        },
        if paso[1] > 0 {
            (y as f32 + 1.0 - origen.y) / dir.y
        } else if paso[1] < 0 {
            (y as f32 - origen.y) / dir.y
        } else {
            f32::INFINITY
        },
        if paso[2] > 0 {
            (z as f32 + 1.0 - origen.z) / dir.z
        } else if paso[2] < 0 {
            (z as f32 - origen.z) / dir.z
        } else {
            f32::INFINITY
        },
    ];

    // Si la camara arranca dentro de un bloque solido, se devuelve sin cara.
    if !bloque(x, y, z).es_aire() {
        return Some(Impacto {
            bloque: [x, y, z],
            normal: [0, 0, 0],
        });
    }

    loop {
        let eje = if siguiente[0] < siguiente[1] && siguiente[0] < siguiente[2] {
            0
        } else if siguiente[1] < siguiente[2] {
            1
        } else {
            2
        };

        if siguiente[eje] > alcance {
            return None;
        }

        // Avanza una celda en ese eje; la normal es la cara por la que entra.
        let normal = match eje {
            0 => {
                x += paso[0];
                [-paso[0], 0, 0]
            }
            1 => {
                y += paso[1];
                [0, -paso[1], 0]
            }
            _ => {
                z += paso[2];
                [0, 0, -paso[2]]
            }
        };
        siguiente[eje] += delta[eje];

        if !bloque(x, y, z).es_aire() {
            return Some(Impacto {
                bloque: [x, y, z],
                normal,
            });
        }
    }
}

/// Bloque vecino en la direccion de la normal: donde se colocaria uno nuevo.
pub fn vecino(impacto: &Impacto) -> [i32; 3] {
    [
        impacto.bloque[0] + impacto.normal[0],
        impacto.bloque[1] + impacto.normal[1],
        impacto.bloque[2] + impacto.normal[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::Chunk;

    fn chunk_con_bloque(x: i32, y: i32, z: i32) -> Chunk {
        let mut chunk = Chunk::new();
        chunk.poner(x, y, z, Bloque::Piedra);
        chunk
    }

    #[test]
    fn un_rayo_vertical_hacia_abajo_impacta_por_arriba() {
        let chunk = chunk_con_bloque(8, 8, 8);
        let impacto = lanzar(
            Vec3::new(8.5, 20.0, 8.5),
            Vec3::new(0.0, -1.0, 0.0),
            30.0,
            |x, y, z| chunk.bloque(x, y, z),
        )
        .expect("deberia impactar");

        assert_eq!(impacto.bloque, [8, 8, 8]);
        assert_eq!(impacto.normal, [0, 1, 0]);
    }

    #[test]
    fn un_rayo_lateral_impacta_por_el_lado_correcto() {
        let chunk = chunk_con_bloque(8, 8, 8);
        let impacto = lanzar(
            Vec3::new(20.0, 8.5, 8.5),
            Vec3::new(-1.0, 0.0, 0.0),
            30.0,
            |x, y, z| chunk.bloque(x, y, z),
        )
        .expect("deberia impactar");

        assert_eq!(impacto.bloque, [8, 8, 8]);
        assert_eq!(impacto.normal, [1, 0, 0]);
    }

    #[test]
    fn el_vecino_es_donde_se_coloca_el_bloque() {
        let impacto = Impacto {
            bloque: [8, 8, 8],
            normal: [0, 1, 0],
        };
        assert_eq!(vecino(&impacto), [8, 9, 8]);
    }

    #[test]
    fn sin_bloques_no_hay_impacto() {
        let chunk = Chunk::new();
        assert_eq!(
            lanzar(
                Vec3::new(8.5, 20.0, 8.5),
                Vec3::new(0.0, -1.0, 0.0),
                30.0,
                |x, y, z| chunk.bloque(x, y, z)
            ),
            None
        );
    }

    #[test]
    fn fuera_de_alcance_no_hay_impacto() {
        let chunk = chunk_con_bloque(8, 8, 8);
        assert_eq!(
            lanzar(
                Vec3::new(8.5, 20.0, 8.5),
                Vec3::new(0.0, -1.0, 0.0),
                5.0,
                |x, y, z| chunk.bloque(x, y, z)
            ),
            None
        );
    }
}
