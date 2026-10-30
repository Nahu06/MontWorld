//! Camara en primera persona.
//!
//! Guarda posicion y angulos (yaw/pitch) y construye las matrices de vista
//! y proyeccion. El movimiento no comprueba colisiones: por ahora es una
//! camara libre para poder explorar la escena de prueba.

use std::f32::consts::FRAC_PI_2;

use crate::math::{Mat4, Vec3};

/// Sensibilidad del raton, en radianes por pixel.
pub const SENSIBILIDAD: f32 = 0.0022;

/// Limite del pitch para no dar la vuelta a la camara.
const LIMITE_PITCH: f32 = FRAC_PI_2 - 0.01;

pub struct Camera {
    pub posicion: Vec3,
    /// Giro horizontal en radianes. 0 mira hacia -Z.
    pub yaw: f32,
    /// Giro vertical en radianes. Positivo mira hacia arriba.
    pub pitch: f32,
    pub fov_y: f32,
    pub cerca: f32,
    pub lejos: f32,
}

impl Camera {
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

    /// Direccion hacia la que mira la camara (incluye la inclinacion).
    pub fn adelante(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// Direccion horizontal de avance, para caminar sin volar al mirar
    /// arriba o abajo.
    pub fn adelante_horizontal(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos())
    }

    pub fn derecha(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin())
    }

    /// Aplica el movimiento del raton.
    pub fn girar(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * SENSIBILIDAD;
        self.pitch = (self.pitch - dy * SENSIBILIDAD).clamp(-LIMITE_PITCH, LIMITE_PITCH);
    }

    pub fn mover(&mut self, desplazamiento: Vec3) {
        self.posicion = self.posicion + desplazamiento;
    }

    /// Orienta la camara hacia un punto del mundo.
    pub fn mirar_a(&mut self, objetivo: Vec3) {
        let direccion = (objetivo - self.posicion).normalize();
        self.yaw = direccion.x.atan2(-direccion.z);
        self.pitch = direccion.y.asin().clamp(-LIMITE_PITCH, LIMITE_PITCH);
    }

    pub fn vista(&self) -> Mat4 {
        // La base se construye a partir de yaw y pitch, y "arriba" sale del
        // producto vectorial: nunca degenera, ni mirando casi al cenit.
        let adelante = self.adelante();
        let derecha = self.derecha();
        let arriba = derecha.cross(adelante);
        Mat4::desde_base(self.posicion, derecha, arriba, adelante)
    }

    pub fn proyeccion(&self, aspecto: f32) -> Mat4 {
        Mat4::perspectiva(self.fov_y, aspecto, self.cerca, self.lejos)
    }

    pub fn vista_proyeccion(&self, aspecto: f32) -> Mat4 {
        // OpenGL multiplica por la izquierda: clip = proyeccion * vista * punto.
        self.proyeccion(aspecto).mul(self.vista())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn distancia_en_vista(cam: &Camera, punto: Vec3) -> f32 {
        let p = transformar(&cam.vista(), punto);
        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
    }

    #[test]
    fn la_base_de_la_camara_es_ortogonal() {
        for pitch in [-1.2, -0.4, 0.0, 0.5, 1.55] {
            let mut cam = Camera::new(Vec3::new(1.0, 2.0, 3.0));
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
            // Mirando hacia arriba, "arriba" de la vista sigue teniendo
            // componente positiva en Y: la imagen no se invierte.
            assert!(arriba.y > 0.0, "pitch {pitch}: arriba = {arriba:?}");
        }
    }

    #[test]
    fn girar_no_acerca_ni_aleja_el_mundo() {
        let punto = Vec3::new(8.0, 8.0, 8.0);
        let mut cam = Camera::new(Vec3::new(8.0, 22.0, 34.0));
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
    fn mirar_a_apunta_al_objetivo() {
        let objetivo = Vec3::new(8.0, 8.0, 8.0);
        let mut cam = Camera::new(Vec3::new(8.0, 22.0, 34.0));
        cam.mirar_a(objetivo);

        let p = transformar(&cam.vista(), objetivo);
        assert!(p[0].abs() < 1e-3 && p[1].abs() < 1e-3, "{p:?}");
        assert!(p[2] < 0.0, "el objetivo debe quedar delante: {p:?}");
    }
}
