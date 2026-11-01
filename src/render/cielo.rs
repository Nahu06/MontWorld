//! Cielo: sol, luna y estrellas.
//!
//! Todo se dibuja a una distancia fija y con una matriz de vista sin
//! traslación, así que los astros se comportan como si estuvieran
//! infinitamente lejos: girar los mueve, caminar no.

use std::error::Error;
use std::f32::consts::TAU;

use crate::base::math::Vec3;

use super::malla::Malla;

/// Distancia a la que se coloca el cielo (dentro del plano lejano).
const DISTANCIA: f32 = 120.0;

/// Cuántas estrellas se reparten por la esfera celeste.
const ESTRELLAS: usize = 220;

/// Cada cuánto (en fracción de día) se reconstruye la geometría.
const PASO_ACTUALIZACION: f32 = 0.0006;

const COLOR_SOL: [f32; 3] = [1.0, 0.95, 0.62];
const COLOR_LUNA: [f32; 3] = [0.88, 0.90, 1.0];
const COLOR_ESTRELLA: [f32; 3] = [0.95, 0.96, 1.0];

pub struct Cielo {
    malla: Malla,
    hora_dibujada: f32,
}

impl Cielo {
    pub fn nuevo(gl: &glow::Context, hora: f32) -> Result<Self, Box<dyn Error>> {
        let malla = Malla::subir(gl, &vertices(hora, 16.0 / 9.0))?;
        Ok(Self {
            malla,
            hora_dibujada: hora,
        })
    }

    /// Reconstruye la geometría cuando el sol se movió lo suficiente.
    pub fn actualizar(
        &mut self,
        gl: &glow::Context,
        hora: f32,
        aspecto: f32,
    ) -> Result<(), Box<dyn Error>> {
        let diferencia = (hora - self.hora_dibujada).abs();
        if diferencia < PASO_ACTUALIZACION && diferencia < 0.5 {
            return Ok(());
        }

        let nueva = Malla::subir(gl, &vertices(hora, aspecto))?;
        let anterior = std::mem::replace(&mut self.malla, nueva);
        anterior.borrar(gl);
        self.hora_dibujada = hora;
        Ok(())
    }

    pub fn dibujar(&self, gl: &glow::Context) {
        self.malla.dibujar(gl);
    }

    pub fn borrar(&self, gl: &glow::Context) {
        self.malla.borrar(gl);
    }
}

/// Dirección del sol para una hora del mundo (0 amanecer, 0.25 mediodía).
pub fn direccion_sol(hora: f32) -> Vec3 {
    let angulo = hora * TAU;
    Vec3::new(angulo.cos(), angulo.sin(), -0.30).normalize()
}

/// Dirección de la luna: siempre opuesta a la del sol.
pub fn direccion_luna(hora: f32) -> Vec3 {
    direccion_sol(hora) * -1.0
}

/// Factor de día (0 de noche, 1 a pleno día), compartido con el color
/// del cielo y la luz global.
pub fn claridad(hora: f32) -> f32 {
    ((hora * TAU).sin() * 1.7).clamp(0.0, 1.0)
}

fn vertices(hora: f32, aspecto: f32) -> Vec<f32> {
    let aspecto = aspecto.max(0.1);
    let mut datos = Vec::new();

    let sol = direccion_sol(hora);
    let luna = direccion_luna(hora);
    let claridad = claridad(hora);

    // Estrellas: puntitos, solo cuando ya oscurece.
    if claridad < 0.35 {
        for i in 0..ESTRELLAS {
            let (direccion, tamano) = estrella(i);
            if direccion.y < -0.05 {
                continue;
            }
            empujar_cuadrado(&mut datos, direccion, tamano / aspecto, tamano, COLOR_ESTRELLA);
        }
    }

    // Sol cuadrado, mientras esté sobre el horizonte.
    if sol.y > -0.12 {
        empujar_cuadrado(&mut datos, sol, 0.055 / aspecto, 0.055, COLOR_SOL);
    }

    // Luna redonda.
    if luna.y > -0.12 {
        empujar_circulo(&mut datos, luna, 0.032, aspecto, 20, COLOR_LUNA);
    }

    datos
}

/// Posición y tamaño de una estrella, deterministas por índice.
fn estrella(indice: usize) -> (Vec3, f32) {
    let a = azar(indice as u32 * 3 + 1);
    let b = azar(indice as u32 * 3 + 2);
    let c = azar(indice as u32 * 3 + 3);

    let y = a * 2.0 - 1.0;
    let radio = (1.0 - y * y).max(0.0).sqrt();
    let angulo = b * TAU;

    let direccion = Vec3::new(radio * angulo.cos(), y, radio * angulo.sin());
    let tamano = 0.0025 + c * 0.0035;
    (direccion, tamano)
}

fn azar(semilla: u32) -> f32 {
    let mut h = semilla.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
    h = h ^ (h >> 13);
    (h & 0xffff) as f32 / 65_535.0
}

/// Base del cartel que mira al origen, para dibujar astros de frente.
fn cartel(direccion: Vec3) -> (Vec3, Vec3) {
    let arriba_mundo = if direccion.y.abs() > 0.99 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::Y
    };
    let derecha = direccion.cross(arriba_mundo).normalize();
    let arriba = derecha.cross(direccion);
    (derecha, arriba)
}

fn empujar_cuadrado(
    salida: &mut Vec<f32>,
    direccion: Vec3,
    semiancho: f32,
    semialto: f32,
    color: [f32; 3],
) {
    let centro = direccion * DISTANCIA;
    let (derecha, arriba) = cartel(direccion);
    let dx = derecha * semiancho;
    let dy = arriba * semialto;

    let esquinas = [
        centro - dx - dy,
        centro + dx - dy,
        centro + dx + dy,
        centro - dx + dy,
    ];

    empujar_quad(salida, esquinas, color);
}

fn empujar_circulo(
    salida: &mut Vec<f32>,
    direccion: Vec3,
    radio: f32,
    aspecto: f32,
    lados: usize,
    color: [f32; 3],
) {
    let centro = direccion * DISTANCIA;
    let (derecha, arriba) = cartel(direccion);
    // El ancho se divide por el aspecto para que la luna se vea redonda.
    let dx = derecha * (radio / aspecto.max(0.1));
    let dy = arriba * radio;

    for i in 0..lados {
        let a0 = (i as f32 / lados as f32) * TAU;
        let a1 = ((i + 1) as f32 / lados as f32) * TAU;

        let p0 = centro + dx * a0.cos() + dy * a0.sin();
        let p1 = centro + dx * a1.cos() + dy * a1.sin();
        for punto in [centro, p0, p1] {
            salida.extend_from_slice(&[punto.x, punto.y, punto.z, color[0], color[1], color[2]]);
        }
    }
}

fn empujar_quad(salida: &mut Vec<f32>, esquinas: [Vec3; 4], color: [f32; 3]) {
    let orden = [0usize, 1, 2, 0, 2, 3];
    for indice in orden {
        let p = esquinas[indice];
        salida.extend_from_slice(&[p.x, p.y, p.z, color[0], color[1], color[2]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertices_por_color(datos: &[f32], color: [f32; 3]) -> usize {
        datos
            .chunks(6)
            .filter(|v| (v[3] - color[0]).abs() < 1e-5 && (v[4] - color[1]).abs() < 1e-5)
            .count()
    }

    #[test]
    fn el_sol_y_la_luna_estan_en_lados_opuestos() {
        for hora in [0.0, 0.1, 0.3, 0.5, 0.8] {
            let sol = direccion_sol(hora);
            let luna = direccion_luna(hora);
            assert!((sol.dot(luna) + 1.0).abs() < 1e-4);
            assert!((sol.length() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn de_noche_hay_estrellas_y_de_dia_no() {
        let noche = vertices(0.75, 16.0 / 9.0);
        let dia = vertices(0.25, 16.0 / 9.0);

        let estrellas_noche = vertices_por_color(&noche, COLOR_ESTRELLA);
        let estrellas_dia = vertices_por_color(&dia, COLOR_ESTRELLA);

        assert!(estrellas_noche > 0, "debería haber estrellas de noche");
        assert_eq!(estrellas_dia, 0, "de día no se dibujan estrellas");
    }

    #[test]
    fn el_sol_es_un_cuadrado_de_cuatro_esquinas() {
        let datos = vertices(0.25, 16.0 / 9.0);
        let vertices_sol = vertices_por_color(&datos, COLOR_SOL);
        // Un cuadrado son 2 triángulos = 6 vértices.
        assert_eq!(vertices_sol, 6);
    }

    #[test]
    fn la_luna_es_un_circulo() {
        let datos = vertices(0.75, 16.0 / 9.0);
        let vertices_luna = vertices_por_color(&datos, COLOR_LUNA);
        // 20 lados, un triángulo por lado.
        assert_eq!(vertices_luna, 20 * 3);
    }
}
