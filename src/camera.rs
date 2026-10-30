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

    pub fn vista(&self) -> Mat4 {
        let arriba = if self.pitch.abs() > 1.5 {
            // Cerca del cenit el vector "arriba" del mundo deja de servir.
            self.derecha().cross(self.adelante()) * -1.0
        } else {
            Vec3::Y
        };
        Mat4::mirar_hacia(self.posicion, self.posicion + self.adelante(), arriba)
    }

    pub fn proyeccion(&self, aspecto: f32) -> Mat4 {
        Mat4::perspectiva(self.fov_y, aspecto, self.cerca, self.lejos)
    }

    pub fn vista_proyeccion(&self, aspecto: f32) -> Mat4 {
        // OpenGL multiplica por la izquierda: clip = proyeccion * vista * punto.
        self.proyeccion(aspecto).mul(self.vista())
    }
}
