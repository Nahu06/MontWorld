//! Chunk: una porción cúbica del mundo.
//!
//! Se guarda como una lista plana de bloques de `LADO^3` posiciones. Las
//! coordenadas internas van de 0 a `LADO - 1`.

use super::block::Bloque;
use super::generacion;

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
                let gx = cx * LADO_I + x;
                let gz = cz * LADO_I + z;
                let altura = generacion::altura(gx, gz);

                for y in 0..altura {
                    // Cuevas: se vacía el interior, nunca el fondo ni la
                    // capa de superficie.
                    if y > 0 && y < altura - 1 && generacion::cueva(gx, y, gz) {
                        continue;
                    }

                    let bloque = if y == 0 {
                        Bloque::RocaMadre
                    } else if y == altura - 1 {
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

        chunk.plantar_arboles(cx, cz);
        chunk
    }

    /// Planta los árboles cuya columna cae dentro de este chunk.
    ///
    /// Se exige un margen de 2 bloques al borde para que la copa no se
    /// corte, y así cada árbol pertenece a un único chunk: los vecinos no
    /// lo duplican.
    fn plantar_arboles(&mut self, cx: i32, cz: i32) {
        const MARGEN: i32 = 2;

        for z in MARGEN..LADO_I - MARGEN {
            for x in MARGEN..LADO_I - MARGEN {
                let gx = cx * LADO_I + x;
                let gz = cz * LADO_I + z;
                if !generacion::hay_arbol(gx, gz) {
                    continue;
                }

                let superficie = generacion::altura(gx, gz);
                // Nada de árboles en la playa ni pegados al techo.
                if superficie <= 5 {
                    continue;
                }

                let tronco = generacion::altura_arbol(gx, gz);
                if superficie + tronco + 2 >= LADO_I {
                    continue;
                }
                if self.bloque(x, superficie - 1, z) != Bloque::Hierba {
                    continue;
                }

                for i in 0..tronco {
                    self.poner(x, superficie + i, z, Bloque::Madera);
                }

                // Copa: dos capas anchas y dos más estrechas arriba.
                let centro = superficie + tronco;
                for dy in -2..=1 {
                    let radio = if dy <= 0 { 2 } else { 1 };
                    for dz in -radio..=radio {
                        for dx in -radio..=radio {
                            // No tapar el tronco por dentro.
                            if dx == 0 && dz == 0 && dy < 0 {
                                continue;
                            }
                            let (lx, ly, lz) = (x + dx, centro + dy, z + dz);
                            if self.bloque(lx, ly, lz).es_aire() {
                                self.poner(lx, ly, lz, Bloque::Hojas);
                            }
                        }
                    }
                }
            }
        }
    }
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
    fn hay_arboles_y_la_superficie_sigue_cerrada() {
        let mut maderas = 0;

        for cz in -1..=1 {
            for cx in -1..=1 {
                let chunk = Chunk::generar(cx, cz);

                for z in 0..LADO_I {
                    for x in 0..LADO_I {
                        let gx = cx * LADO_I + x;
                        let gz = cz * LADO_I + z;
                        let altura = generacion::altura(gx, gz);
                        assert!(
                            !chunk.bloque(x, altura - 1, z).es_aire(),
                            "superficie abierta en ({gx}, {gz})"
                        );
                        for y in 0..LADO_I {
                            if chunk.bloque(x, y, z) == Bloque::Madera {
                                maderas += 1;
                            }
                        }
                    }
                }
            }
        }

        assert!(maderas > 0, "debería haber árboles en la zona");
    }

    #[test]
    fn la_generacion_usa_coordenadas_globales() {
        // El chunk (1, 0) empieza en la coordenada global x = 16: su
        // columna local 0 debe coincidir con la altura calculada allí.
        let chunk = Chunk::generar(1, 0);
        for z in 0..LADO_I {
            let altura = generacion::altura(LADO_I, z);
            assert!(!chunk.bloque(0, altura - 1, z).es_aire());
            if altura < LADO_I {
                assert!(chunk.bloque(0, altura, z).es_aire());
            }
        }
    }

    #[test]
    fn el_fondo_del_mundo_es_roca_madre() {
        let chunk = Chunk::generar(0, 0);
        for z in 0..LADO_I {
            for x in 0..LADO_I {
                assert_eq!(chunk.bloque(x, 0, z), Bloque::RocaMadre);
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
