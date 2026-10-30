//! Capa de render: contexto OpenGL, shaders, mallas y dibujo del frame.
//!
//! Este módulo no conoce las reglas del juego: recibe una cámara y las
//! mallas de cada chunk ya construidas desde la capa superior.

mod contexto;
mod malla;
mod shader;

use std::collections::HashMap;
use std::error::Error;

use glow::HasContext;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::base::math::{Mat4, Vec3};
use crate::jugador::Camara;

use contexto::Contexto;
use malla::Malla;

const VERTICE: &str = include_str!("shaders/bloque.vert");
const FRAGMENTO: &str = include_str!("shaders/bloque.frag");

const COLOR_CIELO: [f32; 4] = [0.53, 0.72, 0.92, 1.0];

pub struct Render {
    contexto: Contexto,
    programa: glow::Program,
    u_vista_proy: Option<glow::UniformLocation>,
    mallas: HashMap<(i32, i32), Malla>,
    mira: Malla,
    barra: Malla,
}

impl Render {
    pub fn nuevo(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let contexto = Contexto::nuevo(event_loop)?;
        let programa = shader::programa(&contexto.gl, VERTICE, FRAGMENTO)?;
        let mira = Malla::subir(&contexto.gl, &vertices_mira())?;
        let barra = Malla::subir(&contexto.gl, &[])?;

        Ok(Self {
            contexto,
            programa,
            u_vista_proy: None,
            mallas: HashMap::new(),
            mira,
            barra,
        })
    }

    pub fn ventana(&self) -> &Window {
        self.contexto.ventana()
    }

    pub fn redimensionar(&mut self, ancho: u32, alto: u32) {
        self.contexto.redimensionar(ancho, alto);
    }

    /// Sube (o reemplaza) la malla de un chunk.
    pub fn subir_chunk(
        &mut self,
        clave: (i32, i32),
        datos: &[f32],
    ) -> Result<(), Box<dyn Error>> {
        let nueva = Malla::subir(&self.contexto.gl, datos)?;
        if let Some(anterior) = self.mallas.insert(clave, nueva) {
            anterior.borrar(&self.contexto.gl);
        }
        Ok(())
    }

    /// Elimina de la GPU la malla de un chunk descargado.
    pub fn quitar_chunk(&mut self, clave: (i32, i32)) {
        if let Some(malla) = self.mallas.remove(&clave) {
            malla.borrar(&self.contexto.gl);
        }
    }

    /// Reconstruye la barra de bloques de la parte inferior de la pantalla.
    /// Los colores los decide la capa de juego: aquí no se conoce ningún
    /// tipo de bloque.
    pub fn actualizar_barra(
        &mut self,
        colores: &[Vec3],
        seleccion: usize,
    ) -> Result<(), Box<dyn Error>> {
        let datos = vertices_barra(colores, seleccion, self.contexto.aspecto());
        let nueva = Malla::subir(&self.contexto.gl, &datos)?;
        let anterior = std::mem::replace(&mut self.barra, nueva);
        anterior.borrar(&self.contexto.gl);
        Ok(())
    }

    pub fn dibujar(&mut self, camara: &Camara) {
        let gl = &self.contexto.gl;
        let (ancho, alto) = self.contexto.tamano();
        let vista_proy = camara.vista_proyeccion(self.contexto.aspecto());

        unsafe {
            gl.viewport(0, 0, ancho as i32, alto as i32);
            gl.clear_color(COLOR_CIELO[0], COLOR_CIELO[1], COLOR_CIELO[2], COLOR_CIELO[3]);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.programa));
            if self.u_vista_proy.is_none() {
                self.u_vista_proy = gl.get_uniform_location(self.programa, "u_vista_proy");
            }

            if !self.mallas.is_empty() {
                gl.enable(glow::DEPTH_TEST);
                if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                    gl.uniform_matrix_4_f32_slice(Some(ubicacion), false, &vista_proy.m);
                }

                for malla in self.mallas.values() {
                    malla.dibujar(gl);
                }
                gl.disable(glow::DEPTH_TEST);
            }

            // HUD en coordenadas de pantalla (NDC), siempre encima.
            gl.disable(glow::DEPTH_TEST);
            if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(ubicacion), false, &Mat4::identidad().m);
            }
            self.mira.dibujar(gl);
            self.barra.dibujar(gl);
            gl.enable(glow::DEPTH_TEST);
        }

        self.contexto.intercambiar();
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        for (_, malla) in self.mallas.drain() {
            malla.borrar(&self.contexto.gl);
        }
        self.mira.borrar(&self.contexto.gl);
        self.barra.borrar(&self.contexto.gl);
        unsafe {
            self.contexto.gl.delete_program(self.programa);
        }
    }
}

/// Mira en coordenadas de pantalla (NDC): dos barras cruzadas.
fn vertices_mira() -> Vec<f32> {
    const BRAZO: f32 = 0.022;
    const GROSOR: f32 = 0.0026;
    const COLOR: [f32; 3] = [0.95, 0.97, 1.0];

    let mut datos = Vec::new();
    empujar_rectangulo(&mut datos, -BRAZO, -GROSOR, BRAZO, GROSOR, COLOR);
    empujar_rectangulo(&mut datos, -GROSOR, -BRAZO, GROSOR, BRAZO, COLOR);
    datos
}

/// Barra rápida de bloques: cuatro casillas con el color de cada bloque y
/// un marco blanco en la seleccionada.
///
/// El ancho se divide por el aspecto para que las casillas se vean
/// cuadradas en pantalla.
fn vertices_barra(colores: &[Vec3], seleccion: usize, aspecto: f32) -> Vec<f32> {
    const ALTO: f32 = 0.11;
    const HUECO: f32 = 0.006;
    const BORDE: f32 = 0.005;
    const MARCO: f32 = 0.004;

    let aspecto = aspecto.max(0.1);
    let ancho = ALTO / aspecto;
    let hueco = HUECO / aspecto;
    let borde = BORDE / aspecto;
    let marco = MARCO / aspecto;

    let cantidad = colores.len() as f32;
    let total = cantidad * ancho + (cantidad - 1.0) * hueco;
    let y0 = -0.95;
    let y1 = y0 + ALTO;

    const FONDO: [f32; 3] = [0.10, 0.11, 0.14];
    const SELECCION: [f32; 3] = [0.98, 0.98, 1.0];

    let mut datos = Vec::new();
    let mut x = -total * 0.5;

    for color in colores {
        empujar_rectangulo(
            &mut datos,
            x - borde,
            y0 - borde,
            x + ancho + borde,
            y1 + borde,
            FONDO,
        );
        empujar_rectangulo(
            &mut datos,
            x,
            y0,
            x + ancho,
            y1,
            [color.x, color.y, color.z],
        );
        x += ancho + hueco;
    }

    // Marco de la casilla seleccionada, dibujado al final para que quede
    // por encima del resto.
    let seleccion = seleccion.min(colores.len().saturating_sub(1));
    let x = -total * 0.5 + seleccion as f32 * (ancho + hueco);
    let (mx0, mx1) = (x - marco - 0.002, x + ancho + marco + 0.002);
    let (my0, my1) = (y0 - marco - 0.002, y1 + marco + 0.002);
    let grosor = 0.0035;
    // Arriba y abajo.
    empujar_rectangulo(&mut datos, mx0, my1 - grosor, mx1, my1, SELECCION);
    empujar_rectangulo(&mut datos, mx0, my0, mx1, my0 + grosor, SELECCION);
    // Izquierda y derecha.
    empujar_rectangulo(&mut datos, mx0, my0, mx0 + marco, my1, SELECCION);
    empujar_rectangulo(&mut datos, mx1 - marco, my0, mx1, my1, SELECCION);

    datos
}

fn empujar_rectangulo(
    salida: &mut Vec<f32>,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color: [f32; 3],
) {
    let mut vertice = |x: f32, y: f32| {
        salida.extend_from_slice(&[x, y, 0.0, color[0], color[1], color[2]]);
    };
    vertice(x0, y0);
    vertice(x1, y0);
    vertice(x1, y1);
    vertice(x0, y0);
    vertice(x1, y1);
    vertice(x0, y1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_barra_tiene_una_casilla_por_bloque() {
        let colores = [Vec3::new(1.0, 0.0, 0.0); 6];
        let datos = vertices_barra(&colores, 0, 16.0 / 9.0);
        // 6 casillas × 2 rectángulos (fondo + color) + 4 barras del marco.
        let rectangulos = 6 * 2 + 4;
        assert_eq!(datos.len(), rectangulos * 6 * 6);
    }

    #[test]
    fn la_seleccion_fuera_de_rango_no_se_sale_de_la_barra() {
        let colores = [Vec3::new(0.5, 0.5, 0.5); 6];
        let datos = vertices_barra(&colores, 99, 16.0 / 9.0);
        assert!(!datos.is_empty());
        // Todos los vértices dentro de la pantalla.
        for vertice in datos.chunks(6) {
            assert!(vertice[0].abs() <= 1.0 && vertice[1].abs() <= 1.0);
        }
    }
}
