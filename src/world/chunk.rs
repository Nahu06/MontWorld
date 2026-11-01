//! Chunk: una porción cúbica del mundo.
//!
//! Se guarda como una lista plana de bloques de `LADO^3` posiciones. Las
//! coordenadas internas van de 0 a `LADO - 1`.
//!
//! Los chunks son 3D: `(cx, cy, cz)`. No hay tope de altura ni de
//! profundidad; el mundo se genera a medida que el jugador se mueve.

use super::block::Bloque;
use super::generacion;

/// Bloques por lado del chunk. Al ser potencia de dos, las cuentas son
/// baratas y el chunk ocupa 4096 bloques (4 KiB).
pub const LADO: usize = 16;
pub const LADO_I: i32 = LADO as i32;
const AREA: usize = LADO * LADO;
const VOLUMEN: usize = AREA * LADO;

/// Coordenada de chunk en la rejilla 3D.
pub type Clave = (i32, i32, i32);

pub struct Chunk {
    origen: Clave,
    bloques: Vec<Bloque>,
}

impl Chunk {
    pub fn new(origen: Clave) -> Self {
        Self {
            origen,
            bloques: vec![Bloque::Aire; VOLUMEN],
        }
    }

    pub fn origen(&self) -> Clave {
        self.origen
    }

    fn indice(x: usize, y: usize, z: usize) -> usize {
        x + z * LADO + y * AREA
    }

    /// Lee un bloque en coordenadas locales. Fuera del chunk es aire.
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

    /// Genera el terreno del chunk situado en la rejilla `(cx, cy, cz)`.
    pub fn generar(cx: i32, cy: i32, cz: i32) -> Self {
        let mut chunk = Self::new((cx, cy, cz));
        let base_y = cy * LADO_I;

        for z in 0..LADO_I {
            for x in 0..LADO_I {
                let gx = cx * LADO_I + x;
                let gz = cz * LADO_I + z;
                // Primer bloque de aire de la columna.
                let superficie = generacion::altura(gx, gz);

                for y in 0..LADO_I {
                    let gy = base_y + y;
                    if gy >= superficie {
                        break;
                    }

                    // Cuevas: nunca en la capa de superficie.
                    if gy < superficie - 1 && generacion::cueva(gx, gy, gz) {
                        continue;
                    }

                    let bloque = if gy == superficie - 1 {
                        // Zonas bajas: playa.
                        if superficie <= 0 { Bloque::Arena } else { Bloque::Hierba }
                    } else if gy >= superficie - 3 {
                        Bloque::Tierra
                    } else {
                        Bloque::Piedra
                    };
                    chunk.poner(x, y, z, bloque);
                }
            }
        }

        chunk.plantar_arboles(cx, cy, cz);
        chunk
    }

    /// Planta los árboles cuya columna cae dentro de este chunk.
    ///
    /// Se exige margen al borde horizontal (para no cortar la copa) y que
    /// el árbol entero quepa en la altura del chunk. Así cada árbol
    /// pertenece a un único chunk y no se duplica.
    fn plantar_arboles(&mut self, cx: i32, cy: i32, cz: i32) {
        const MARGEN: i32 = 2;
        let base_y = cy * LADO_I;

        for z in MARGEN..LADO_I - MARGEN {
            for x in MARGEN..LADO_I - MARGEN {
                let gx = cx * LADO_I + x;
                let gz = cz * LADO_I + z;
                if !generacion::hay_arbol(gx, gz) {
                    continue;
                }

                let superficie = generacion::altura(gx, gz);
                if superficie <= 5 {
                    continue;
                }

                let tronco = generacion::altura_arbol(gx, gz);
                let base_local = superficie - base_y;

                // La superficie debe estar en este chunk y el árbol caber.
                if base_local < 1 || base_local + tronco + 2 > LADO_I {
                    continue;
                }
                if self.bloque(x, base_local - 1, z) != Bloque::Hierba {
                    continue;
                }

                for i in 0..tronco {
                    self.poner(x, base_local + i, z, Bloque::Madera);
                }

                // Copa: dos capas anchas y dos más estrechas arriba.
                let centro = base_local + tronco;
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
    fn el_terreno_tiene_bloques_alrededor_de_la_superficie() {
        let mut solidos = 0;
        for cy in -2..=2 {
            let chunk = Chunk::generar(0, cy, 0);
            for y in 0..LADO_I {
                for z in 0..LADO_I {
                    for x in 0..LADO_I {
                        if !chunk.bloque(x, y, z).es_aire() {
                            solidos += 1;
                        }
                    }
                }
            }
        }
        assert!(solidos > 0, "la columna del mundo debería tener terreno");
    }

    #[test]
    fn la_generacion_usa_coordenadas_globales() {
        // El chunk (1, 0, 0) empieza en la coordenada global x = 16: su
        // columna local 0 debe coincidir con la altura calculada allí.
        let chunk = Chunk::generar(1, 0, 0);
        for z in 0..LADO_I {
            let superficie = generacion::altura(LADO_I, z);
            let local = superficie - 1;
            if (0..LADO_I).contains(&local) {
                assert!(!chunk.bloque(0, local, z).es_aire());
            }
        }
    }

    #[test]
    fn la_generacion_es_determinista() {
        let a = Chunk::generar(3, -2, 1);
        let b = Chunk::generar(3, -2, 1);
        for y in 0..LADO_I {
            for z in 0..LADO_I {
                for x in 0..LADO_I {
                    assert_eq!(a.bloque(x, y, z), b.bloque(x, y, z));
                }
            }
        }
    }

    #[test]
    fn muy_por_debajo_del_suelo_sigue_habiendo_piedra() {
        // Chunk a 160 bloques de profundidad: debe tener material.
        let chunk = Chunk::generar(0, -10, 0);
        let solidos = (0..LADO_I)
            .flat_map(|y| (0..LADO_I).flat_map(move |z| (0..LADO_I).map(move |x| (x, y, z))))
            .filter(|(x, y, z)| !chunk.bloque(*x, *y, *z).es_aire())
            .count();
        assert!(solidos > 0, "el subsuelo profundo no debería estar vacío");
    }

    #[test]
    fn hay_arboles_y_la_superficie_sigue_cerrada() {
        let mut maderas = 0;

        for cz in -4..=4 {
            for cx in -4..=4 {
                for cy in -3..=3 {
                    let chunk = Chunk::generar(cx, cy, cz);
                    let base_y = cy * LADO_I;

                    for z in 0..LADO_I {
                        for x in 0..LADO_I {
                            let gx = cx * LADO_I + x;
                            let gz = cz * LADO_I + z;
                            let superficie = generacion::altura(gx, gz);
                            let local = superficie - 1 - base_y;
                            if (0..LADO_I).contains(&local) {
                                assert!(
                                    !chunk.bloque(x, local, z).es_aire(),
                                    "superficie abierta en ({gx}, {gz})"
                                );
                            }

                            for y in 0..LADO_I {
                                if chunk.bloque(x, y, z) == Bloque::Madera {
                                    maderas += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(maderas > 0, "debería haber árboles en la zona");
    }
}
