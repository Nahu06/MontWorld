//! El mundo: coleccion de chunks y acceso a bloques por coordenadas
//! globales.
//!
//! Los chunks se cargan y descargan en una rejilla alrededor del jugador.
//! El terreno solo ocupa los 16 bloques de alto de un chunk por ahora;
//! apilar chunks en vertical queda para mas adelante.

use std::collections::HashMap;

use super::block::Bloque;
use super::chunk::{Chunk, LADO_I};
use super::meshing;

pub struct Mundo {
    radio: i32,
    chunks: HashMap<(i32, i32), Chunk>,
}

impl Mundo {
    pub fn new(radio: i32) -> Self {
        Self {
            radio,
            chunks: HashMap::new(),
        }
    }

    /// Chunk al que pertenece una coordenada global horizontal.
    pub fn clave_de(x: i32, z: i32) -> (i32, i32) {
        (x.div_euclid(LADO_I), z.div_euclid(LADO_I))
    }

    pub fn bloque(&self, x: i32, y: i32, z: i32) -> Bloque {
        if y < 0 || y >= LADO_I {
            return Bloque::Aire;
        }
        let Some(chunk) = self.chunks.get(&Self::clave_de(x, z)) else {
            return Bloque::Aire;
        };
        chunk.bloque(x.rem_euclid(LADO_I), y, z.rem_euclid(LADO_I))
    }

    /// Cambia un bloque. Devuelve false si esa zona no esta cargada.
    pub fn poner(&mut self, x: i32, y: i32, z: i32, bloque: Bloque) -> bool {
        if y < 0 || y >= LADO_I {
            return false;
        }
        let Some(chunk) = self.chunks.get_mut(&Self::clave_de(x, z)) else {
            return false;
        };
        chunk.poner(x.rem_euclid(LADO_I), y, z.rem_euclid(LADO_I), bloque);
        true
    }

    pub fn tiene_chunk(&self, clave: (i32, i32)) -> bool {
        self.chunks.contains_key(&clave)
    }

    /// Construye la malla de un chunk teniendo en cuenta a sus vecinos.
    pub fn malla_de(&self, clave: (i32, i32)) -> Vec<f32> {
        let Some(chunk) = self.chunks.get(&clave) else {
            return Vec::new();
        };
        let (cx, cz) = clave;
        let origen = [
            (cx * LADO_I) as f32,
            0.0,
            (cz * LADO_I) as f32,
        ];
        meshing::malla_con(chunk, origen, |x, y, z| {
            self.bloque(cx * LADO_I + x, y, cz * LADO_I + z)
        })
    }

    /// Carga los chunks que faltan dentro del radio del centro y descarta
    /// los que quedan fuera. Devuelve (nuevos, descartados).
    pub fn ajustar(&mut self, centro: (i32, i32)) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
        let mut deseados = Vec::with_capacity(((self.radio * 2 + 1) * (self.radio * 2 + 1)) as usize);
        for dz in -self.radio..=self.radio {
            for dx in -self.radio..=self.radio {
                deseados.push((centro.0 + dx, centro.1 + dz));
            }
        }

        let mut quitados = Vec::new();
        for clave in self.chunks.keys().copied().collect::<Vec<_>>() {
            if !deseados.contains(&clave) {
                self.chunks.remove(&clave);
                quitados.push(clave);
            }
        }

        let mut nuevos = Vec::new();
        for clave in deseados {
            if !self.chunks.contains_key(&clave) {
                self.chunks
                    .insert(clave, Chunk::generar(clave.0, clave.1));
                nuevos.push(clave);
            }
        }

        (nuevos, quitados)
    }

    pub fn cantidad(&self) -> usize {
        self.chunks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_coordenadas_negativas_caen_en_el_chunk_correcto() {
        assert_eq!(Mundo::clave_de(0, 0), (0, 0));
        assert_eq!(Mundo::clave_de(15, 15), (0, 0));
        assert_eq!(Mundo::clave_de(16, 0), (1, 0));
        assert_eq!(Mundo::clave_de(-1, -1), (-1, -1));
        assert_eq!(Mundo::clave_de(-17, -17), (-2, -2));
    }

    #[test]
    fn fuera_de_los_chunks_cargados_hay_aire() {
        let mundo = Mundo::new(0);
        assert_eq!(mundo.bloque(0, 0, 0), Bloque::Aire);
    }

    #[test]
    fn ajustar_carga_la_rejilla_y_descarta_lo_lejano() {
        let mut mundo = Mundo::new(1);
        let (nuevos, quitados) = mundo.ajustar((0, 0));
        assert_eq!(nuevos.len(), 9);
        assert!(quitados.is_empty());

        let (nuevos, quitados) = mundo.ajustar((1, 0));
        assert_eq!(nuevos.len(), 3);
        assert_eq!(quitados.len(), 3);
        assert_eq!(mundo.cantidad(), 9);
    }

    #[test]
    fn cada_malla_se_coloca_en_su_posicion_global() {
        let mut mundo = Mundo::new(0);
        mundo.ajustar((1, 0));
        mundo.poner(20, 5, 20, Bloque::Piedra);

        let datos = mundo.malla_de((1, 0));
        assert!(!datos.is_empty());
        for vertice in datos.chunks(6) {
            assert!(
                vertice[0] >= 16.0 && vertice[0] <= 32.0,
                "vertice fuera del chunk: x = {}",
                vertice[0]
            );
            assert!(vertice[2] >= 0.0 && vertice[2] <= 16.0);
        }
    }

    #[test]
    fn un_bloque_puesto_se_lee_en_coordenadas_globales() {
        let mut mundo = Mundo::new(1);
        mundo.ajustar((0, 0));
        assert!(mundo.poner(20, 5, 20, Bloque::Piedra));
        assert_eq!(mundo.bloque(20, 5, 20), Bloque::Piedra);

        let clave = Mundo::clave_de(20, 20);
        let datos = mundo.malla_de(clave);
        assert!(!datos.is_empty());
    }
}
