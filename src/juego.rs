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

/// Altura a la que reaparece el jugador al caer al vacío: por encima
/// de todo el mundo cargado, para que vuelva a caer desde arriba.
const ALTURA_REAPARICION: f32 = 20.0;

/// Bloques disponibles en la barra rápida, en orden (teclas 1 a 4).
const BLOQUES_BARRA: [Bloque; 4] = [
    Bloque::Hierba,
    Bloque::Tierra,
    Bloque::Piedra,
    Bloque::Arena,
];

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
    /// Índice del bloque seleccionado en la barra rápida.
    seleccion: usize,
    /// Punto de reaparición si el jugador cae al vacío.
    aparicion: Vec3,
}

impl Juego {
    pub fn nuevo() -> Self {
        // Aparecer justo encima del terreno, en el centro del mundo.
        let aparicion = Vec3::new(8.5, generacion::altura(8, 8) as f32 + 0.2, 8.5);
        Self {
            mundo: Mundo::new(2),
            jugador: Jugador::nuevo(aparicion),
            centro: None,
            cambios: Cambios::default(),
            seleccion: 2, // piedra por defecto
            aparicion,
        }
    }

    /// Un paso de simulación.
    pub fn actualizar(&mut self, entrada: &mut Input, paso: f32) {
        {
            let solido = |x, y, z| !self.mundo.bloque(x, y, z).es_aire();
            self.jugador.actualizar(entrada, paso, &solido);
        }

        // Muerte por caída al vacío: reaparece arriba del todo, en la
        // columna del punto inicial, y vuelve a caer sobre el terreno.
        if self.jugador.posicion.y < -20.0 {
            println!("Montworld: caíste al vacío; reapareciendo arriba");
            let reaparicion = Vec3::new(
                self.aparicion.x,
                ALTURA_REAPARICION,
                self.aparicion.z,
            );
            self.jugador
                .colocar_en(reaparicion, self.jugador.camara.yaw, 0.0);
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

    /// Elige un bloque de la barra por su índice (0 a 3).
    pub fn seleccionar(&mut self, indice: usize) {
        self.seleccion = indice.min(BLOQUES_BARRA.len() - 1);
    }

    /// Pasa al bloque anterior o siguiente de la barra.
    pub fn ciclar_seleccion(&mut self, adelante: bool) {
        let total = BLOQUES_BARRA.len();
        self.seleccion = if adelante {
            (self.seleccion + 1) % total
        } else {
            (self.seleccion + total - 1) % total
        };
    }

    pub fn seleccion(&self) -> usize {
        self.seleccion
    }

    pub fn bloque_seleccionado(&self) -> Bloque {
        BLOQUES_BARRA[self.seleccion]
    }

    /// Colores de los bloques de la barra, para que el render los dibuje
    /// sin conocer los tipos de bloque.
    pub fn colores_barra(&self) -> [Vec3; 4] {
        BLOQUES_BARRA.map(|bloque| bloque.color())
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

        // No colocar un bloque dentro del cuerpo del jugador: se quedaría
        // atrapado. Se comprueba la caja completa, no solo la cámara.
        if self.jugador.ocupa(x, y, z) {
            return false;
        }

        if !self.mundo.bloque(x, y, z).es_aire() {
            return false;
        }

        self.mundo.poner(x, y, z, self.bloque_seleccionado())
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

    /// Coloca al jugador sobre el terreno mirando al suelo.
    fn mirar_al_suelo(juego: &mut Juego, pies: f32) {
        juego.jugador.posicion = Vec3::new(8.5, pies, 8.5);
        juego.jugador.velocidad = Vec3::ZERO;
        juego.jugador.sincronizar_camara();
        juego.jugador.camara.yaw = 0.0;
        juego.jugador.camara.pitch = -1.5;
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
        mirar_al_suelo(&mut juego, (altura + 2) as f32);

        assert!(juego.romper());
        assert!(!juego.tomar_sucios().is_empty());
    }

    #[test]
    fn colocar_usa_el_bloque_seleccionado() {
        let mut juego = juego_con_bloque();
        juego.tomar_cambios();

        let altura = crate::world::generacion::altura(8, 8);
        mirar_al_suelo(&mut juego, (altura + 2) as f32);

        juego.seleccionar(0); // hierba
        assert_eq!(juego.bloque_seleccionado(), Bloque::Hierba);
        assert!(juego.colocar());

        assert!(
            juego.mundo.cambios().values().any(|b| *b == Bloque::Hierba),
            "debería haberse colocado hierba"
        );
    }

    #[test]
    fn la_seleccion_cicla_en_los_dos_sentidos() {
        let mut juego = Juego::nuevo();
        juego.seleccionar(0);
        juego.ciclar_seleccion(false);
        assert_eq!(juego.seleccion(), 3);
        juego.ciclar_seleccion(true);
        assert_eq!(juego.seleccion(), 0);

        juego.seleccionar(99);
        assert_eq!(juego.seleccion(), 3);
    }

    #[test]
    fn caer_al_vacio_devuelve_al_inicio() {
        let mut juego = juego_con_bloque();
        let mut entrada = Input::new();

        juego.jugador.posicion = Vec3::new(8.5, -100.0, 8.5);
        juego.actualizar(&mut entrada, 1.0 / 60.0);

        assert!(
            (juego.jugador.posicion.y - ALTURA_REAPARICION).abs() < 0.01,
            "debería reaparecer arriba: y = {}",
            juego.jugador.posicion.y
        );
    }

    #[test]
    fn no_se_puede_colocar_un_bloque_dentro_del_jugador() {
        let mut juego = juego_con_bloque();
        juego.tomar_cambios();

        // De pie sobre el terreno y mirando al suelo: el bloque que se
        // colocaría quedaría en la celda de los pies.
        let altura = crate::world::generacion::altura(8, 8);
        mirar_al_suelo(&mut juego, altura as f32);

        assert!(!juego.colocar(), "no debería colocar dentro del jugador");
        assert!(juego.mundo.cambios().is_empty());
    }

    #[test]
    fn colocar_requiere_aire_y_cara_valida() {
        let mut juego = juego_con_bloque();
        juego.tomar_cambios();
        let altura = crate::world::generacion::altura(8, 8);
        mirar_al_suelo(&mut juego, (altura + 2) as f32);

        assert!(juego.colocar());
        assert!(!juego.tomar_sucios().is_empty());
    }
}
