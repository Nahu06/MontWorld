//! El mundo: colección de chunks 3D y acceso a bloques por coordenadas
//! globales.
//!
//! No hay tope de altura ni de profundidad: los chunks se cargan y
//! descargan en una rejilla 3D alrededor del jugador.
//!
//! El mundo lleva la cuenta de los chunks "sucios" (los que hay que
//! reconstruir en la GPU tras un cambio de bloques).

use std::collections::{HashMap, HashSet};

use super::block::Bloque;
use super::chunk::{Chunk, Clave, LADO_I};
use super::guardado::Cambios;
use super::meshing;

pub struct Mundo {
    /// Radio horizontal de carga, en chunks.
    radio: i32,
    /// Radio vertical de carga, en chunks.
    radio_vertical: i32,
    chunks: HashMap<Clave, Chunk>,
    sucios: HashSet<Clave>,
    /// Bloques modificados respecto al terreno generado.
    cambios: Cambios,
}

impl Mundo {
    pub fn new(radio: i32, radio_vertical: i32) -> Self {
        Self::con_cambios(radio, radio_vertical, Cambios::new())
    }

    /// Mundo que arranca con una lista de bloques ya modificados.
    pub fn con_cambios(radio: i32, radio_vertical: i32, cambios: Cambios) -> Self {
        Self {
            radio,
            radio_vertical,
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

    /// Chunk al que pertenece una coordenada global.
    pub fn clave_de(x: i32, y: i32, z: i32) -> Clave {
        (
            x.div_euclid(LADO_I),
            y.div_euclid(LADO_I),
            z.div_euclid(LADO_I),
        )
    }

    pub fn bloque(&self, x: i32, y: i32, z: i32) -> Bloque {
        let Some(chunk) = self.chunks.get(&Self::clave_de(x, y, z)) else {
            return Bloque::Aire;
        };
        chunk.bloque(
            x.rem_euclid(LADO_I),
            y.rem_euclid(LADO_I),
            z.rem_euclid(LADO_I),
        )
    }

    /// Cambia un bloque. Devuelve false si esa zona no está cargada.
    pub fn poner(&mut self, x: i32, y: i32, z: i32, bloque: Bloque) -> bool {
        let Some(chunk) = self.chunks.get_mut(&Self::clave_de(x, y, z)) else {
            return false;
        };
        chunk.poner(
            x.rem_euclid(LADO_I),
            y.rem_euclid(LADO_I),
            z.rem_euclid(LADO_I),
            bloque,
        );
        self.cambios.insert((x, y, z), bloque);

        self.marcar_sucio(x, y, z);
        true
    }

    /// Apunta los chunks cuya malla cambia al tocar el bloque `(x, y, z)`.
    /// Si el bloque está en una cara del chunk, también cambia el vecino.
    fn marcar_sucio(&mut self, x: i32, y: i32, z: i32) {
        let (cx, cy, cz) = Self::clave_de(x, y, z);
        self.sucios.insert((cx, cy, cz));

        let lx = x.rem_euclid(LADO_I);
        let ly = y.rem_euclid(LADO_I);
        let lz = z.rem_euclid(LADO_I);

        if lx == 0 {
            self.sucios.insert((cx - 1, cy, cz));
        }
        if lx == LADO_I - 1 {
            self.sucios.insert((cx + 1, cy, cz));
        }
        if ly == 0 {
            self.sucios.insert((cx, cy - 1, cz));
        }
        if ly == LADO_I - 1 {
            self.sucios.insert((cx, cy + 1, cz));
        }
        if lz == 0 {
            self.sucios.insert((cx, cy, cz - 1));
        }
        if lz == LADO_I - 1 {
            self.sucios.insert((cx, cy, cz + 1));
        }
    }

    /// Devuelve los chunks pendientes de reconstruir y vacía la lista.
    pub fn tomar_sucios(&mut self) -> Vec<Clave> {
        let mut claves: Vec<_> = self.sucios.drain().collect();
        claves.sort_unstable();
        claves
    }

    pub fn tiene_chunk(&self, clave: Clave) -> bool {
        self.chunks.contains_key(&clave)
    }

    /// Construye la malla de un chunk teniendo en cuenta a sus vecinos.
    pub fn malla_de(&self, clave: Clave) -> Vec<f32> {
        let Some(chunk) = self.chunks.get(&clave) else {
            return Vec::new();
        };
        let (cx, cy, cz) = chunk.origen();
        let origen = [
            (cx * LADO_I) as f32,
            (cy * LADO_I) as f32,
            (cz * LADO_I) as f32,
        ];
        meshing::malla_con(chunk, origen, |x, y, z| {
            self.bloque(cx * LADO_I + x, cy * LADO_I + y, cz * LADO_I + z)
        })
    }

    /// Carga los chunks que faltan dentro de los radios del centro y
    /// descarta los que quedan fuera.
    ///
    /// `presupuesto` limita cuántos chunks se generan en esta llamada:
    /// el resto queda pendiente para los siguientes frames, así no se
    /// producen tirones al cruzar un borde. Se generan primero los más
    /// cercanos al jugador. Devuelve (nuevos, descartados).
    pub fn ajustar(&mut self, centro: Clave, presupuesto: usize) -> (Vec<Clave>, Vec<Clave>) {
        let horizontal = (self.radio * 2 + 1) as usize;
        let vertical = (self.radio_vertical * 2 + 1) as usize;
        let mut deseados = Vec::with_capacity(horizontal * horizontal * vertical);

        for dy in -self.radio_vertical..=self.radio_vertical {
            for dz in -self.radio..=self.radio {
                for dx in -self.radio..=self.radio {
                    deseados.push((centro.0 + dx, centro.1 + dy, centro.2 + dz));
                }
            }
        }

        let conjunto: HashSet<Clave> = deseados.iter().copied().collect();

        let mut quitados = Vec::new();
        for clave in self.chunks.keys().copied().collect::<Vec<_>>() {
            if !conjunto.contains(&clave) {
                self.chunks.remove(&clave);
                self.sucios.remove(&clave);
                quitados.push(clave);
            }
        }

        // Primero los chunks más cercanos (distancia Manhattan).
        deseados.sort_by_key(|c| {
            (c.0 - centro.0).abs() + (c.1 - centro.1).abs() + (c.2 - centro.2).abs()
        });

        let mut nuevos = Vec::new();
        for clave in deseados {
            if nuevos.len() >= presupuesto {
                break;
            }
            if !self.chunks.contains_key(&clave) {
                let mut chunk = Chunk::generar(clave.0, clave.1, clave.2);

                // Reaplicar los bloques que el jugador ya había cambiado.
                for ((x, y, z), bloque) in &self.cambios {
                    if Self::clave_de(*x, *y, *z) == clave {
                        chunk.poner(
                            x.rem_euclid(LADO_I),
                            y.rem_euclid(LADO_I),
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
        assert_eq!(Mundo::clave_de(0, 0, 0), (0, 0, 0));
        assert_eq!(Mundo::clave_de(15, 15, 15), (0, 0, 0));
        assert_eq!(Mundo::clave_de(16, 16, 16), (1, 1, 1));
        assert_eq!(Mundo::clave_de(-1, -1, -1), (-1, -1, -1));
        assert_eq!(Mundo::clave_de(-17, -17, -17), (-2, -2, -2));
    }

    #[test]
    fn fuera_de_los_chunks_cargados_hay_aire() {
        let mundo = Mundo::new(0, 0);
        assert_eq!(mundo.bloque(0, 0, 0), Bloque::Aire);
    }

    #[test]
    fn ajustar_carga_la_rejilla_3d_y_descarta_lo_lejano() {
        let mut mundo = Mundo::new(1, 1);
        let (nuevos, quitados) = mundo.ajustar((0, 0, 0), 100);
        assert_eq!(nuevos.len(), 27);
        assert!(quitados.is_empty());

        let (nuevos, quitados) = mundo.ajustar((1, 0, 0), 100);
        assert_eq!(nuevos.len(), 9);
        assert_eq!(quitados.len(), 9);
        assert_eq!(mundo.cantidad(), 27);
    }

    #[test]
    fn un_bloque_puesto_se_lee_en_coordenadas_globales() {
        let mut mundo = Mundo::new(1, 1);
        mundo.ajustar((0, 0, 0), 100);
        assert!(mundo.poner(20, 5, 20, Bloque::Piedra));
        assert_eq!(mundo.bloque(20, 5, 20), Bloque::Piedra);

        let clave = Mundo::clave_de(20, 5, 20);
        assert!(!mundo.malla_de(clave).is_empty());
    }

    #[test]
    fn el_presupuesto_reparte_la_generacion_entre_llamadas() {
        let mut mundo = Mundo::new(1, 1);

        let (nuevos, _) = mundo.ajustar((0, 0, 0), 1);
        assert_eq!(nuevos.len(), 1, "solo debe generar un chunk");
        assert_eq!(nuevos[0], (0, 0, 0), "primero el más cercano");

        let (nuevos, _) = mundo.ajustar((0, 0, 0), 4);
        assert_eq!(nuevos.len(), 4);
        assert!(!nuevos.contains(&(0, 0, 0)), "no debe repetir el ya cargado");
        assert!(mundo.cantidad() < 27);

        mundo.ajustar((0, 0, 0), 100);
        assert_eq!(mundo.cantidad(), 27);
    }

    #[test]
    fn se_puede_poner_y_leer_muy_en_profundo() {
        let mut mundo = Mundo::new(0, 2);
        mundo.ajustar((0, -10, 0), 100);
        assert!(mundo.poner(5, -155, 5, Bloque::Piedra));
        assert_eq!(mundo.bloque(5, -155, 5), Bloque::Piedra);
    }

    #[test]
    fn cada_malla_se_coloca_en_su_posicion_global() {
        // Elegir el chunk que contiene la superficie en (20, 20).
        let altura = super::super::generacion::altura(20, 20);
        let clave = Mundo::clave_de(20, altura, 20);

        let mut mundo = Mundo::new(1, 1);
        mundo.ajustar(clave, 100);
        assert!(mundo.poner(20, altura, 20, Bloque::Piedra));

        let datos = mundo.malla_de(clave);
        assert!(!datos.is_empty());

        let (cx, cy, cz) = clave;
        let (x0, y0, z0) = (
            (cx * LADO_I) as f32,
            (cy * LADO_I) as f32,
            (cz * LADO_I) as f32,
        );
        for vertice in datos.chunks(6) {
            assert!(vertice[0] >= x0 && vertice[0] <= x0 + 16.0);
            assert!(vertice[1] >= y0 && vertice[1] <= y0 + 16.0);
            assert!(vertice[2] >= z0 && vertice[2] <= z0 + 16.0);
        }
    }

    #[test]
    fn un_cambio_marca_sucio_el_chunk_y_los_vecinos_del_borde() {
        let mut mundo = Mundo::new(1, 1);
        mundo.ajustar((0, 0, 0), 100);

        mundo.poner(5, 5, 5, Bloque::Piedra);
        assert_eq!(mundo.tomar_sucios(), vec![(0, 0, 0)]);

        mundo.poner(0, 5, 5, Bloque::Piedra);
        let sucios = mundo.tomar_sucios();
        assert!(sucios.contains(&(0, 0, 0)));
        assert!(sucios.contains(&(-1, 0, 0)));
    }

    #[test]
    fn los_cambios_se_reaplican_al_recargar_el_chunk() {
        let mut mundo = Mundo::new(0, 1);
        mundo.ajustar((0, 0, 0), 100);
        mundo.poner(5, 10, 5, Bloque::Piedra);
        mundo.poner(6, 10, 5, Bloque::Aire);

        let cambios = mundo.cambios().clone();
        let mut recargado = Mundo::con_cambios(0, 1, cambios);
        recargado.ajustar((0, 0, 0), 100);

        assert_eq!(recargado.bloque(5, 10, 5), Bloque::Piedra);
        assert_eq!(recargado.bloque(6, 10, 5), Bloque::Aire);
    }
}
