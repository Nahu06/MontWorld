//! Matematicas minimas del motor: vectores 3D y matrices 4x4.
//!
//! Se escriben a mano para no depender de ninguna libreria externa.
//! Las matrices se guardan en columnas (column-major), que es el formato
//! que espera OpenGL.

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, otro: Self) -> f32 {
        self.x * otro.x + self.y * otro.y + self.z * otro.z
    }

    pub fn cross(self, otro: Self) -> Self {
        Self::new(
            self.y * otro.z - self.z * otro.y,
            self.z * otro.x - self.x * otro.z,
            self.x * otro.y - self.y * otro.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Self {
        let largo = self.length();
        if largo > 0.0 { self * (1.0 / largo) } else { self }
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, otro: Self) -> Self {
        Self::new(self.x + otro.x, self.y + otro.y, self.z + otro.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, otro: Self) -> Self {
        Self::new(self.x - otro.x, self.y - otro.y, self.z - otro.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, escalar: f32) -> Self {
        Self::new(self.x * escalar, self.y * escalar, self.z * escalar)
    }
}

/// Matriz 4x4 en columnas: `m[columna * 4 + fila]`.
#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Mat4 {
    /// Proyeccion en perspectiva. `fov_y` va en radianes.
    pub fn perspectiva(fov_y: f32, aspecto: f32, cerca: f32, lejos: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let inv_rango = 1.0 / (cerca - lejos);
        Self {
            m: [
                f / aspecto, 0.0, 0.0, 0.0,
                0.0, f, 0.0, 0.0,
                0.0, 0.0, (lejos + cerca) * inv_rango, -1.0,
                0.0, 0.0, 2.0 * lejos * cerca * inv_rango, 0.0,
            ],
        }
    }

    /// Matriz de vista mirando desde `ojo` hacia `objetivo`.
    pub fn mirar_hacia(ojo: Vec3, objetivo: Vec3, arriba: Vec3) -> Self {
        let f = (objetivo - ojo).normalize();
        let s = f.cross(arriba).normalize();
        let u = s.cross(f);
        Self {
            m: [
                s.x, s.y, s.z, 0.0,
                u.x, u.y, u.z, 0.0,
                -f.x, -f.y, -f.z, 0.0,
                -s.dot(ojo), -u.dot(ojo), f.dot(ojo), 1.0,
            ],
        }
    }

    /// Producto `self * otra`.
    pub fn mul(self, otra: Self) -> Self {
        let mut salida = [0.0f32; 16];
        for col in 0..4 {
            for fila in 0..4 {
                let mut suma = 0.0;
                for k in 0..4 {
                    suma += self.m[k * 4 + fila] * otra.m[col * 4 + k];
                }
                salida[col * 4 + fila] = suma;
            }
        }
        Self { m: salida }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transforma un punto por una matriz (mismo calculo que hace la GPU).
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

    #[test]
    fn la_vista_lleva_el_ojo_al_origen() {
        let ojo = Vec3::new(3.0, 4.0, 5.0);
        let vista = Mat4::mirar_hacia(ojo, ojo + Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let p = transformar(&vista, ojo);
        assert!(p[0].abs() < 1e-5 && p[1].abs() < 1e-5 && p[2].abs() < 1e-5);
    }

    #[test]
    fn un_punto_delante_queda_en_z_negativa() {
        let ojo = Vec3::new(0.0, 2.0, 8.0);
        let vista = Mat4::mirar_hacia(ojo, ojo + Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let p = transformar(&vista, Vec3::new(0.0, 2.0, 0.0));
        assert!((p[2] + 8.0).abs() < 1e-5, "z = {}", p[2]);
        assert!(p[0].abs() < 1e-5 && p[1].abs() < 1e-5);
    }

    #[test]
    fn mirar_a_la_derecha_deja_el_punto_a_la_izquierda() {
        let ojo = Vec3::new(0.0, 0.0, 0.0);
        let vista = Mat4::mirar_hacia(ojo, Vec3::new(1.0, 0.0, 0.0), Vec3::Y);
        // Un punto al norte (+Z) del ojo debe quedar a la izquierda (-X).
        let p = transformar(&vista, Vec3::new(0.0, 0.0, 1.0));
        assert!(p[0] < -0.99, "x = {}", p[0]);
    }

    #[test]
    fn la_perspectiva_deja_el_centro_en_pantalla() {
        let proy = Mat4::perspectiva(70.0f32.to_radians(), 16.0 / 9.0, 0.1, 300.0);
        let p = transformar(&proy, Vec3::new(0.0, 0.0, -5.0));
        assert!(p[3] > 0.0, "w = {}", p[3]);
        let ndc_x = p[0] / p[3];
        let ndc_y = p[1] / p[3];
        let ndc_z = p[2] / p[3];
        assert!(ndc_x.abs() < 1e-6 && ndc_y.abs() < 1e-6);
        assert!((0.0..1.0).contains(&ndc_z), "z = {ndc_z}");
    }

    #[test]
    fn la_vista_y_la_proyeccion_se_combinan_en_orden() {
        let camara = Vec3::new(0.0, 2.0, 8.0);
        let vista = Mat4::mirar_hacia(camara, camara + Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let proy = Mat4::perspectiva(70.0f32.to_radians(), 16.0 / 9.0, 0.1, 300.0);
        // Orden correcto para OpenGL: proyeccion * vista.
        let vista_proy = proy.mul(vista);

        let p = transformar(&vista_proy, Vec3::new(0.0, 2.0, 0.0));
        let ndc_x = p[0] / p[3];
        let ndc_y = p[1] / p[3];
        let ndc_z = p[2] / p[3];

        // Un cubo justo enfrente, en el centro de la pantalla.
        assert!(ndc_x.abs() < 1e-5 && ndc_y.abs() < 1e-5);
        assert!((-1.0..1.0).contains(&ndc_z), "z = {ndc_z}");
    }
}
