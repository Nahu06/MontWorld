//! El mundo: colección de chunks y acceso a bloques por coordenadas
//! globales.
//!
//! Los chunks se cargan y descargan en una rejilla alrededor del jugador.
//! El terreno solo ocupa los 16 bloques de alto de un chunk por ahora;
//! apilar chunks en vertical queda para más adelante.
//!
//! El mundo lleva la cuenta de los chunks "sucios" (los que hay que
//! reconstruir en la GPU tras un cambio de bloques).

use std::collections::{HashMap, HashSet};

use super::block::Bloque;
use super::chunk::{Chunk, LADO_I};
use super::guardado::Cambios;
use super::meshing;

/// Altura del lecho rocoso: no se puede romper ni cambiar.
pub const NIVEL_ROCA_MADRE: i32 = 0;

pub struct Mundo {
    radio: i32,
    chunks: HashMap<(i32, i32), Chunk>,
    sucios: HashSet<(i32, i32)>,
    /// Bloques modificados respecto al terreno generado.
    cambios: Cambios,
}

impl Mundo {
    pub fn new(radio: i32) -> Self {
        Self::con_cambios(radio, Cambios::new())
    }

    /// Mundo que arranca con una lista de bloques ya modificados.
    pub fn con_cambios(radio: i32, cambios: Cambios) -> Self {
        Self {
            radio,
            chunks: HashMap::new(),
            sucios: HashSet::new(),
            cambios,
        }
    }

    /// Bloques modificados, para guardar la partida.
    pub fn cambios(&self) -> &Cambios {
        &self.cambios
    }

    /// Sustituye los cambios y olvida los chunks cargados: se regenerarán
    /// al volver a pedirlos, ya con los cambios aplicados.
    pub fn reemplazar_cambios(&mut self, cambios: Cambios) {
        self.cambios = cambios;
        self.chunks.clear();
        self.sucios.clear();
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

    /// Cambia un bloque. Devuelve false si esa zona no está cargada o si
    /// es el lecho rocoso del fondo del mundo (irrompible).
    pub fn poner(&mut self, x: i32, y: i32, z: i32, bloque: Bloque) -> bool {
        if y <= NIVEL_ROCA_MADRE || y >= LADO_I {
            return false;
        }
        let Some(chunk) = self.chunks.get_mut(&Self::clave_de(x, z)) else {
            return false;
        };
        chunk.poner(x.rem_euclid(LADO_I), y, z.rem_euclid(LADO_I), bloque);
        self.cambios.insert((x, y, z), bloque);

        self.marcar_sucio(x, z);
        true
    }

    /// Apunta los chunks cuya malla cambia al tocar el bloque `(x, z)`.
    /// Si el bloque está en un borde, también cambia la malla del vecino.
    fn marcar_sucio(&mut self, x: i32, z: i32) {
        let (cx, cz) = Self::clave_de(x, z);
        self.sucios.insert((cx, cz));

        let lx = x.rem_euclid(LADO_I);
        let lz = z.rem_euclid(LADO_I);
        if lx == 0 {
            self.sucios.insert((cx - 1, cz));
        }
        if lx == LADO_I - 1 {
            self.sucios.insert((cx + 1, cz));
        }
        if lz == 0 {
            self.sucios.insert((cx, cz - 1));
        }
        if lz == LADO_I - 1 {
            self.sucios.insert((cx, cz + 1));
        }
    }

    /// Devuelve los chunks pendientes de reconstruir y vacía la lista.
    pub fn tomar_sucios(&mut self) -> Vec<(i32, i32)> {
        let mut claves: Vec<_> = self.sucios.drain().collect();
        claves.sort_unstable();
        claves
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
        let origen = [(cx * LADO_I) as f32, 0.0, (cz * LADO_I) as f32];
        meshing::malla_con(chunk, origen, |x, y, z| {
            self.bloque(cx * LADO_I + x, y, cz * LADO_I + z)
        })
    }

    /// Carga los chunks que faltan dentro del radio del centro y descarta
    /// los que quedan fuera. Devuelve (nuevos, descartados).
    pub fn ajustar(&mut self, centro: (i32, i32)) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
        let lado = (self.radio * 2 + 1) as usize;
        let mut deseados = Vec::with_capacity(lado * lado);
        for dz in -self.radio..=self.radio {
            for dx in -self.radio..=self.radio {
                deseados.push((centro.0 + dx, centro.1 + dz));
            }
        }

        let mut quitados = Vec::new();
        for clave in self.chunks.keys().copied().collect::<Vec<_>>() {
            if !deseados.contains(&clave) {
                self.chunks.remove(&clave);
                self.sucios.remove(&clave);
                quitados.push(clave);
            }
        }

        let mut nuevos = Vec::new();
        for clave in deseados {
            if !self.chunks.contains_key(&clave) {
                let mut chunk = Chunk::generar(clave.0, clave.1);

                // Reaplicar los bloques que el jugador ya había cambiado.
                for ((x, y, z), bloque) in &self.cambios {
                    // Partidas anteriores pudieron guardar agujeros en el
                    // fondo: se ignoran para que el lecho quede intacto.
                    if *y <= NIVEL_ROCA_MADRE {
                        continue;
                    }
                    if Self::clave_de(*x, *z) == clave {
                        chunk.poner(
                            x.rem_euclid(LADO_I),
                            *y,
                            z.rem_euclid(LADO_I),
                            *bloque,
                        );
                    }
                }

                self.chunks.insert(clave, chunk);
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
    fn un_bloque_puesto_se_lee_en_coordenadas_globales() {
        let mut mundo = Mundo::new(1);
        mundo.ajustar((0, 0));
        assert!(mundo.poner(20, 5, 20, Bloque::Piedra));
        assert_eq!(mundo.bloque(20, 5, 20), Bloque::Piedra);

        let clave = Mundo::clave_de(20, 20);
        let datos = mundo.malla_de(clave);
        assert!(!datos.is_empty());
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
                "vértice fuera del chunk: x = {}",
                vertice[0]
            );
            assert!(vertice[2] >= 0.0 && vertice[2] <= 16.0);
        }
    }

    #[test]
    fn el_lecho_rocoso_no_se_puede_romper() {
        let mut mundo = Mundo::new(0);
        mundo.ajustar((0, 0));
        assert!(!mundo.poner(5, NIVEL_ROCA_MADRE, 5, Bloque::Aire));
        assert_eq!(mundo.bloque(5, NIVEL_ROCA_MADRE, 5), Bloque::RocaMadre);
    }

    #[test]
    fn los_cambios_viejos_en_el_fondo_no_abren_agujeros() {
        let mut cambios = Cambios::new();
        cambios.insert((3, NIVEL_ROCA_MADRE, 4), Bloque::Aire);

        let mut mundo = Mundo::con_cambios(0, cambios);
        mundo.ajustar((0, 0));
        assert_eq!(mundo.bloque(3, NIVEL_ROCA_MADRE, 4), Bloque::RocaMadre);
    }

    #[test]
    fn los_cambios_se_reaplican_al_recargar_el_chunk() {
        let mut mundo = Mundo::new(0);
        mundo.ajustar((0, 0));
        mundo.poner(5, 10, 5, Bloque::Piedra);
        mundo.poner(6, 10, 5, Bloque::Aire);

        let cambios = mundo.cambios().clone();
        let mut recargado = Mundo::con_cambios(0, cambios);
        recargado.ajustar((0, 0));

        assert_eq!(recargado.bloque(5, 10, 5), Bloque::Piedra);
        assert_eq!(recargado.bloque(6, 10, 5), Bloque::Aire);
    }

    #[test]
    fn un_cambio_marca_sucio_el_chunk_y_el_vecino_del_borde() {
        let mut mundo = Mundo::new(1);
        mundo.ajustar((0, 0));

        mundo.poner(5, 5, 5, Bloque::Piedra);
        assert_eq!(mundo.tomar_sucios(), vec![(0, 0)]);

        mundo.poner(0, 5, 5, Bloque::Piedra);
        let sucios = mundo.tomar_sucios();
        assert!(sucios.contains(&(0, 0)));
        assert!(sucios.contains(&(-1, 0)));
    }
}
