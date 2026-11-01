//! Jugador: cuerpo con física simple y cámara en primera persona.
//!
//! El cuerpo es una caja (AABB) que cae por gravedad, salta y choca con
//! los bloques sólidos. Para no depender de `world`, la comprobación de
//! solidez se recibe como función: `(x, y, z) -> ¿hay bloque?`.
//!
//! La cámara va montada a la altura de los ojos del cuerpo.

use std::f32::consts::FRAC_PI_2;

use winit::keyboard::KeyCode;

use crate::base::math::{Mat4, Vec3};
use crate::entrada::Input;

/// Sensibilidad del ratón, en radianes por píxel.
pub const SENSIBILIDAD: f32 = 0.0022;

/// Límite del pitch para no dar la vuelta a la cámara.
const LIMITE_PITCH: f32 = FRAC_PI_2 - 0.01;

/// Medidas del cuerpo, en bloques.
pub const ANCHO: f32 = 0.6;
pub const ALTO: f32 = 1.8;
pub const ALTURA_OJOS: f32 = 1.62;

/// Física del jugador.
const GRAVEDAD: f32 = 28.0;
const CAIDA_MAXIMA: f32 = 40.0;
const VELOCIDAD_CAMINAR: f32 = 5.5;
const VELOCIDAD_CORRER: f32 = 9.0;
const IMPULSO_SALTO: f32 = 8.5;

/// Margen para no detectar colisión con el bloque que se acaba de tocar.
const MARGEN: f32 = 0.001;

pub struct Camara {
    pub posicion: Vec3,
    /// Giro horizontal en radianes. 0 mira hacia -Z.
    pub yaw: f32,
    /// Giro vertical en radianes. Positivo mira hacia arriba.
    pub pitch: f32,
    pub fov_y: f32,
    pub cerca: f32,
    pub lejos: f32,
}

impl Camara {
    pub fn new(posicion: Vec3) -> Self {
        Self {
            posicion,
            yaw: 0.0,
            pitch: 0.0,
            fov_y: 70.0f32.to_radians(),
            cerca: 0.1,
            lejos: 300.0,
        }
    }

    /// Dirección hacia la que mira (incluye la inclinación).
    pub fn adelante(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// Dirección horizontal de avance, para caminar sin mirar al cielo.
    pub fn adelante_horizontal(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn derecha(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    /// Aplica el movimiento del ratón.
    pub fn girar(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * SENSIBILIDAD;
        self.pitch = (self.pitch - dy * SENSIBILIDAD).clamp(-LIMITE_PITCH, LIMITE_PITCH);
    }

    /// Orienta la cámara hacia un punto del mundo.
    /// Útil para vistas de cámara fijas (tests y futuras escenas).
    #[allow(dead_code)]
    pub fn mirar_a(&mut self, objetivo: Vec3) {
        let direccion = (objetivo - self.posicion).normalize();
        self.yaw = direccion.x.atan2(-direccion.z);
        self.pitch = direccion.y.asin().clamp(-LIMITE_PITCH, LIMITE_PITCH);
    }

    pub fn vista(&self) -> Mat4 {
        Mat4::desde_base(
            self.posicion,
            self.derecha(),
            self.derecha().cross(self.adelante()),
            self.adelante(),
        )
    }

    /// Vista con la misma orientación pero sin traslación.
    ///
    /// Se usa para el cielo: al ignorar la posición del jugador, los
    /// astros se comportan como si estuvieran en el infinito y caminar no
    /// los mueve.
    pub fn vista_rotacion(&self) -> Mat4 {
        Mat4::desde_base(
            Vec3::ZERO,
            self.derecha(),
            self.derecha().cross(self.adelante()),
            self.adelante(),
        )
    }

    pub fn proyeccion(&self, aspecto: f32) -> Mat4 {
        Mat4::perspectiva(self.fov_y, aspecto, self.cerca, self.lejos)
    }

    pub fn vista_proyeccion(&self, aspecto: f32) -> Mat4 {
        // OpenGL multiplica por la izquierda: clip = proyección * vista * p.
        self.proyeccion(aspecto).mul(self.vista())
    }
}

pub struct Jugador {
    pub camara: Camara,
    /// Posición de los pies (centro de la base del cuerpo).
    pub posicion: Vec3,
    pub velocidad: Vec3,
    pub en_suelo: bool,
}

impl Jugador {
    pub fn nuevo(posicion: Vec3) -> Self {
        let mut jugador = Self {
            camara: Camara::new(posicion + Vec3::new(0.0, ALTURA_OJOS, 0.0)),
            posicion,
            velocidad: Vec3::ZERO,
            en_suelo: false,
        };
        jugador.sincronizar_camara();
        jugador
    }

    /// Coloca el cuerpo y la cámara en un estado concreto (al cargar una
    /// partida).
    pub fn colocar_en(&mut self, posicion: Vec3, yaw: f32, pitch: f32) {
        self.posicion = posicion;
        self.velocidad = Vec3::ZERO;
        self.en_suelo = false;
        self.camara.yaw = yaw;
        self.camara.pitch = pitch.clamp(-LIMITE_PITCH, LIMITE_PITCH);
        self.sincronizar_camara();
    }

    /// Copia la posición del cuerpo a la cámara (a la altura de los ojos).
    pub fn sincronizar_camara(&mut self) {
        self.camara.posicion = self.posicion + Vec3::new(0.0, ALTURA_OJOS, 0.0);
    }

    /// Solo aplica el giro del ratón (sin física). Se usa mientras el
    /// mundo todavía no tiene cargado el chunk del jugador.
    pub fn mirar(&mut self, entrada: &mut Input) {
        let (dx, dy) = entrada.tomar_raton();
        if dx != 0.0 || dy != 0.0 {
            self.camara.girar(dx, dy);
        }
    }

    /// Un paso de control y física. `solido` dice si una celda es sólida.
    pub fn actualizar(
        &mut self,
        entrada: &mut Input,
        paso: f32,
        solido: &dyn Fn(i32, i32, i32) -> bool,
    ) {
        self.mirar(entrada);

        // Velocidad horizontal: se fija directamente para que el control
        // responda al instante.
        let mut direccion = Vec3::ZERO;
        if entrada.abajo(KeyCode::KeyW) {
            direccion = direccion + self.camara.adelante_horizontal();
        }
        if entrada.abajo(KeyCode::KeyS) {
            direccion = direccion - self.camara.adelante_horizontal();
        }
        if entrada.abajo(KeyCode::KeyD) {
            direccion = direccion + self.camara.derecha();
        }
        if entrada.abajo(KeyCode::KeyA) {
            direccion = direccion - self.camara.derecha();
        }

        let correr = entrada.abajo(KeyCode::ShiftLeft) || entrada.abajo(KeyCode::ShiftRight);
        let velocidad = if correr {
            VELOCIDAD_CORRER
        } else {
            VELOCIDAD_CAMINAR
        };
        let horizontal = if direccion.length() > 0.0 {
            direccion.normalize() * velocidad
        } else {
            Vec3::ZERO
        };
        self.velocidad.x = horizontal.x;
        self.velocidad.z = horizontal.z;

        // Salto y gravedad.
        if self.en_suelo && entrada.abajo(KeyCode::Space) {
            self.velocidad.y = IMPULSO_SALTO;
            self.en_suelo = false;
        }
        self.velocidad.y = (self.velocidad.y - GRAVEDAD * paso).max(-CAIDA_MAXIMA);

        // Si por algún motivo quedó dentro de un bloque (una partida
        // vieja, un bloque colocado encima), se libera antes de moverse.
        self.desatascar(solido);

        // Movimiento resuelto eje por eje para poder deslizar por paredes.
        self.mover_eje(0, paso, solido);
        self.mover_eje(2, paso, solido);
        self.mover_eje(1, paso, solido);

        self.sincronizar_camara();
    }

    /// Avanza un eje y deshace el movimiento si acaba dentro de un bloque.
    fn mover_eje(&mut self, eje: usize, paso: f32, solido: &dyn Fn(i32, i32, i32) -> bool) {
        let anterior = self.posicion;
        let delta = self.velocidad.get(eje) * paso;
        self.posicion = self.posicion.con_eje(eje, self.posicion.get(eje) + delta);

        if !self.colisiona(solido) {
            if eje == 1 {
                self.en_suelo = false;
            }
            return;
        }

        self.posicion = anterior;
        if eje == 1 {
            // Si chocamos bajando, estamos apoyados en el suelo.
            self.en_suelo = self.velocidad.y < 0.0;
        }
        self.velocidad = self.velocidad.con_eje(eje, 0.0);
    }

    /// Indica si el cuerpo ocuparía la celda `(x, y, z)`.
    ///
    /// Se usa para no permitir colocar bloques dentro del jugador. Tocar
    /// el borde no cuenta: se puede colocar justo debajo de los pies.
    pub fn ocupa(&self, x: i32, y: i32, z: i32) -> bool {
        let min = self.posicion + Vec3::new(-ANCHO * 0.5, 0.0, -ANCHO * 0.5);
        let max = self.posicion + Vec3::new(ANCHO * 0.5, ALTO, ANCHO * 0.5);

        min.x < x as f32 + 1.0 - MARGEN
            && max.x > x as f32 + MARGEN
            && min.y < y as f32 + 1.0 - MARGEN
            && max.y > y as f32 + MARGEN
            && min.z < z as f32 + 1.0 - MARGEN
            && max.z > z as f32 + MARGEN
    }

    /// Si el cuerpo está dentro de bloques, lo sube hasta sacarlo.
    fn desatascar(&mut self, solido: &dyn Fn(i32, i32, i32) -> bool) {
        let mut intentos = 0;
        while self.colisiona(solido) && intentos < 12 {
            self.posicion.y += 0.25;
            intentos += 1;
        }
    }

    /// Comprueba si la caja del cuerpo se solapa con algún bloque sólido.
    fn colisiona(&self, solido: &dyn Fn(i32, i32, i32) -> bool) -> bool {
        let min = self.posicion + Vec3::new(-ANCHO * 0.5 + MARGEN, MARGEN, -ANCHO * 0.5 + MARGEN);
        let max = self.posicion + Vec3::new(ANCHO * 0.5 - MARGEN, ALTO - MARGEN, ANCHO * 0.5 - MARGEN);

        let x0 = min.x.floor() as i32;
        let y0 = min.y.floor() as i32;
        let z0 = min.z.floor() as i32;
        let x1 = max.x.floor() as i32;
        let y1 = max.y.floor() as i32;
        let z1 = max.z.floor() as i32;

        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if solido(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Suelo a partir de y = 3 (el bloque y = 2 es el último sólido).
    fn suelo(_x: i32, y: i32, _z: i32) -> bool {
        y < 3
    }

    fn transformar(m: &Mat4, p: Vec3) -> [f32; 4] {
        let mut salida = [0.0f32; 4];
        for fila in 0..4 {
            salida[fila] = m.m[fila] * p.x
                + m.m[4 + fila] * p.y
                + m.m[8 + fila] * p.z
                + m.m[12 + fila];
        }
        salida
    }

    fn distancia_en_vista(cam: &Camara, punto: Vec3) -> f32 {
        let p = transformar(&cam.vista(), punto);
        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
    }

    fn avanzar(jugador: &mut Jugador, entrada: &mut Input, pasos: usize, solido: &dyn Fn(i32, i32, i32) -> bool) {
        for _ in 0..pasos {
            jugador.actualizar(entrada, 1.0 / 60.0, solido);
        }
    }

    #[test]
    fn la_base_de_la_camara_es_ortogonal() {
        for pitch in [-1.2, -0.4, 0.0, 0.5, 1.55] {
            let mut cam = Camara::new(Vec3::new(1.0, 2.0, 3.0));
            cam.yaw = 0.8;
            cam.pitch = pitch;

            let adelante = cam.adelante();
            let derecha = cam.derecha();
            let arriba = derecha.cross(adelante);

            assert!(adelante.dot(derecha).abs() < 1e-5);
            assert!(adelante.dot(arriba).abs() < 1e-5);
            assert!(derecha.dot(arriba).abs() < 1e-5);
            assert!((adelante.length() - 1.0).abs() < 1e-5);
            assert!((arriba.length() - 1.0).abs() < 1e-5);
            assert!(arriba.y > 0.0, "pitch {pitch}: arriba = {arriba:?}");
        }
    }

    #[test]
    fn girar_no_acerca_ni_aleja_el_mundo() {
        let punto = Vec3::new(8.0, 8.0, 8.0);
        let mut cam = Camara::new(Vec3::new(8.0, 22.0, 34.0));
        let referencia = distancia_en_vista(&cam, punto);

        for (yaw, pitch) in [(0.0, 0.0), (0.7, 0.3), (2.4, -0.6), (3.0, 1.5), (-1.2, -1.55)] {
            cam.yaw = yaw;
            cam.pitch = pitch;
            let distancia = distancia_en_vista(&cam, punto);
            assert!(
                (distancia - referencia).abs() < 1e-3,
                "yaw {yaw}, pitch {pitch}: {distancia} != {referencia}"
            );
        }
    }

    #[test]
    fn el_cielo_no_se_mueve_al_caminar() {
        let mut cerca = Camara::new(Vec3::new(0.0, 5.0, 0.0));
        let mut lejos = Camara::new(Vec3::new(300.0, -20.0, 120.0));
        cerca.yaw = 0.9;
        cerca.pitch = 0.2;
        lejos.yaw = 0.9;
        lejos.pitch = 0.2;

        let punto = Vec3::new(0.0, 100.0, -60.0);
        let a = transformar(&cerca.vista_rotacion(), punto);
        let b = transformar(&lejos.vista_rotacion(), punto);

        for i in 0..3 {
            assert!((a[i] - b[i]).abs() < 1e-4, "el cielo se movió: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn mirar_a_apunta_al_objetivo() {
        let objetivo = Vec3::new(8.0, 8.0, 8.0);
        let mut cam = Camara::new(Vec3::new(8.0, 22.0, 34.0));
        cam.mirar_a(objetivo);

        let p = transformar(&cam.vista(), objetivo);
        assert!(p[0].abs() < 1e-3 && p[1].abs() < 1e-3, "{p:?}");
        assert!(p[2] < 0.0, "el objetivo debe quedar delante: {p:?}");
    }

    #[test]
    fn el_cuerpo_cae_y_se_apoya_en_el_suelo() {
        let mut jugador = Jugador::nuevo(Vec3::new(0.5, 10.0, 0.5));
        let mut entrada = Input::new();
        avanzar(&mut jugador, &mut entrada, 240, &suelo);

        assert!(jugador.en_suelo, "debería estar apoyado");
        assert!(
            (jugador.posicion.y - 3.0).abs() < 0.02,
            "y = {} (esperado ~3)",
            jugador.posicion.y
        );
    }

    #[test]
    fn el_salto_despega_del_suelo_y_vuelve_a_caer() {
        let mut jugador = Jugador::nuevo(Vec3::new(0.5, 3.0, 0.5));
        let mut entrada = Input::new();
        avanzar(&mut jugador, &mut entrada, 30, &suelo);
        assert!(jugador.en_suelo);

        entrada.presionar(KeyCode::Space);
        jugador.actualizar(&mut entrada, 1.0 / 60.0, &suelo);
        entrada.soltar(KeyCode::Space);

        assert!(!jugador.en_suelo);
        assert!(jugador.velocidad.y > 0.0);

        avanzar(&mut jugador, &mut entrada, 120, &suelo);
        assert!(jugador.en_suelo, "debería volver a caer");
    }

    #[test]
    fn el_cuerpo_ocupa_sus_celdas_pero_no_la_de_debajo() {
        let jugador = Jugador::nuevo(Vec3::new(0.5, 3.0, 0.5));
        // Pies y torso dentro de la misma columna.
        assert!(jugador.ocupa(0, 3, 0));
        assert!(jugador.ocupa(0, 4, 0));
        // La celda del suelo está justo debajo de los pies.
        assert!(!jugador.ocupa(0, 2, 0));
        // Al lado no ocupa nada.
        assert!(!jugador.ocupa(1, 3, 0));
    }

    #[test]
    fn si_queda_atrapado_sale_hacia_arriba() {
        // Suelo y un bloque encima del jugador, a su misma altura.
        let atrapado = |x: i32, y: i32, z: i32| y < 3 || (x == 0 && z == 0 && y == 3);
        let mut jugador = Jugador::nuevo(Vec3::new(0.5, 3.0, 0.5));
        let mut entrada = Input::new();

        assert!(jugador.colisiona(&atrapado));
        jugador.actualizar(&mut entrada, 1.0 / 60.0, &atrapado);
        assert!(!jugador.colisiona(&atrapado), "debería haber salido del bloque");
    }

    #[test]
    fn una_pared_frena_el_avance() {
        // Pared sólida para x >= 5, sobre el suelo.
        let pared = |x: i32, y: i32, _z: i32| y < 3 || x >= 5;
        let mut jugador = Jugador::nuevo(Vec3::new(0.5, 3.0, 0.5));
        let mut entrada = Input::new();

        jugador.camara.yaw = FRAC_PI_2;
        entrada.presionar(KeyCode::KeyW);
        avanzar(&mut jugador, &mut entrada, 240, &pared);

        assert!(jugador.en_suelo);
        assert!(
            jugador.posicion.x < 5.0,
            "atravesó la pared: x = {}",
            jugador.posicion.x
        );
        assert!(jugador.posicion.x > 4.0, "no llegó a la pared: x = {}", jugador.posicion.x);
    }
}
