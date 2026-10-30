//! Simulación del juego: une el mundo con el jugador y define las reglas
//! de interacción (romper y colocar bloques).
//!
//! No sabe nada de OpenGL ni de winit. Cuando modifica el mundo deja
//! apuntados los chunks que hay que reconstruir, y la capa `app` se
//! encarga de subirlos a la GPU.

use crate::base::math::Vec3;
use crate::entrada::Input;
use crate::jugador::Jugador;
use crate::world::block::Bloque;
use crate::world::guardado::Partida;
use crate::world::{Mundo, generacion, raycast};

/// Alcance del rayo para romper y colocar bloques, en bloques.
const ALCANCE: f32 = 6.0;

/// Cambios de chunks pendientes de aplicar en la GPU.
#[derive(Default)]
pub struct Cambios {
    pub nuevos: Vec<(i32, i32)>,
    pub quitados: Vec<(i32, i32)>,
}

pub struct Juego {
    pub mundo: Mundo,
    pub jugador: Jugador,
    /// Chunk en el que está la cámara, para saber cuándo recargar.
    centro: Option<(i32, i32)>,
    cambios: Cambios,
}

impl Juego {
    pub fn nuevo() -> Self {
        Self {
            mundo: Mundo::new(2),
            // Aparecer justo encima del terreno, en el centro del mundo.
            jugador: Jugador::nuevo(Vec3::new(
                8.5,
                generacion::altura(8, 8) as f32 + 0.2,
                8.5,
            )),
            centro: None,
            cambios: Cambios::default(),
        }
    }

    /// Un paso de simulación.
    pub fn actualizar(&mut self, entrada: &mut Input, paso: f32) {
        {
            let solido = |x, y, z| !self.mundo.bloque(x, y, z).es_aire();
            self.jugador.actualizar(entrada, paso, &solido);
        }
        self.actualizar_chunks();
    }

    /// Carga y descarga chunks alrededor del jugador. Solo hace trabajo
    /// cuando cambia de chunk.
    fn actualizar_chunks(&mut self) {
        let p = self.jugador.camara.posicion;
        let centro = Mundo::clave_de(p.x.floor() as i32, p.z.floor() as i32);
        if self.centro == Some(centro) {
            return;
        }
        self.centro = Some(centro);

        let (nuevos, quitados) = self.mundo.ajustar(centro);
        self.cambios.nuevos.extend(nuevos);
        self.cambios.quitados.extend(quitados);
    }

    /// Fuerza una recarga completa en el próximo paso (por ejemplo, al
    /// volver a crear el contexto gráfico).
    pub fn reiniciar_streaming(&mut self) {
        self.centro = None;
        self.actualizar_chunks();
    }

    /// Estado actual para guardar la partida.
    pub fn partida(&self) -> Partida {
        let p = self.jugador.posicion;
        Partida {
            posicion: [p.x, p.y, p.z],
            yaw: self.jugador.camara.yaw,
            pitch: self.jugador.camara.pitch,
            cambios: self.mundo.cambios().clone(),
        }
    }

    /// Carga una partida: recoloca al jugador y aplica sus cambios.
    /// Los chunks se recargan en el siguiente paso.
    pub fn cargar(&mut self, partida: Partida) {
        self.jugador.colocar_en(
            Vec3::new(
                partida.posicion[0],
                partida.posicion[1],
                partida.posicion[2],
            ),
            partida.yaw,
            partida.pitch,
        );
        self.mundo.reemplazar_cambios(partida.cambios);
        self.centro = None;
    }

    /// Retira los cambios de chunks pendientes de aplicar.
    pub fn tomar_cambios(&mut self) -> Cambios {
        std::mem::take(&mut self.cambios)
    }

    /// Chunks que hay que reconstruir tras un cambio de bloques.
    pub fn tomar_sucios(&mut self) -> Vec<(i32, i32)> {
        self.mundo.tomar_sucios()
    }

    /// Quita el bloque al que apunta la mira. Devuelve si hubo cambio.
    pub fn romper(&mut self) -> bool {
        let Some(impacto) = self.apuntar() else {
            return false;
        };
        let [x, y, z] = impacto.bloque;
        self.mundo.poner(x, y, z, Bloque::Aire)
    }

    /// Coloca un bloque en la cara apuntada. Devuelve si hubo cambio.
    pub fn colocar(&mut self) -> bool {
        let Some(impacto) = self.apuntar() else {
            return false;
        };

        // Si el rayo nace dentro de un bloque no hay cara donde colocar.
        if impacto.normal == [0, 0, 0] {
            return false;
        }

        let [x, y, z] = raycast::vecino(&impacto);

        // No colocar un bloque donde está el jugador.
        let p = self.jugador.camara.posicion;
        let ocupada = p.x.floor() as i32 == x
            && p.y.floor() as i32 == y
            && p.z.floor() as i32 == z;
        if ocupada {
            return false;
        }

        if !self.mundo.bloque(x, y, z).es_aire() {
            return false;
        }

        self.mundo.poner(x, y, z, Bloque::Piedra)
    }

    fn apuntar(&self) -> Option<raycast::Impacto> {
        let camara = &self.jugador.camara;
        raycast::lanzar(camara.posicion, camara.adelante(), ALCANCE, |x, y, z| {
            self.mundo.bloque(x, y, z)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn juego_con_bloque() -> Juego {
        let mut juego = Juego::nuevo();
        juego.reiniciar_streaming();
        juego
    }

    #[test]
    fn al_arrancar_se_cargan_los_chunks_alrededor() {
        let mut juego = juego_con_bloque();
        let cambios = juego.tomar_cambios();
        assert_eq!(cambios.nuevos.len(), 25);
        assert!(cambios.quitados.is_empty());
        assert_eq!(juego.mundo.cantidad(), 25);
    }

    #[test]
    fn romper_deja_aire_y_marca_el_chunk_sucio() {
        let mut juego = juego_con_bloque();
        juego.tomar_cambios();

        // Mirar hacia abajo desde un par de bloques sobre el terreno.
        let altura = crate::world::generacion::altura(8, 8);
        juego.jugador.camara.posicion = Vec3::new(8.5, (altura + 2) as f32, 8.5);
        juego.jugador.camara.yaw = 0.0;
        juego.jugador.camara.pitch = -1.5;

        assert!(juego.romper());
        assert!(!juego.tomar_sucios().is_empty());
    }

    #[test]
    fn colocar_requiere_aire_y_cara_valida() {
        let mut juego = juego_con_bloque();
        juego.tomar_cambios();
        let altura = crate::world::generacion::altura(8, 8);
        juego.jugador.camara.posicion = Vec3::new(8.5, (altura + 2) as f32, 8.5);
        juego.jugador.camara.yaw = 0.0;
        juego.jugador.camara.pitch = -1.5;

        assert!(juego.colocar());
        assert!(!juego.tomar_sucios().is_empty());
    }
}
