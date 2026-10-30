//! Capa de render: contexto OpenGL, shaders, mallas y dibujo del frame.
//!
//! Este módulo no conoce las reglas del juego: recibe una cámara y le
//! pide a la capa superior las mallas de cada chunk.

mod contexto;
mod malla;
mod shader;

use std::collections::HashMap;
use std::error::Error;

use glow::HasContext;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::base::math::Mat4;
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
}

impl Render {
    pub fn nuevo(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        let contexto = Contexto::nuevo(event_loop)?;
        let programa = shader::programa(&contexto.gl, VERTICE, FRAGMENTO)?;
        let mira = Malla::subir(&contexto.gl, &vertices_mira())?;

        Ok(Self {
            contexto,
            programa,
            u_vista_proy: None,
            mallas: HashMap::new(),
            mira,
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

    pub fn dibujar(&mut self, camara: &Camara) {
        let gl = &self.contexto.gl;
        let (ancho, alto) = self.contexto.tamano();
        let vista_proy = camara.vista_proyeccion(self.contexto.aspecto());

        unsafe {
            gl.viewport(0, 0, ancho as i32, alto as i32);
            gl.clear_color(COLOR_CIELO[0], COLOR_CIELO[1], COLOR_CIELO[2], COLOR_CIELO[3]);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            if !self.mallas.is_empty() {
                gl.enable(glow::DEPTH_TEST);
                gl.use_program(Some(self.programa));

                if self.u_vista_proy.is_none() {
                    self.u_vista_proy = gl.get_uniform_location(self.programa, "u_vista_proy");
                }
                if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                    gl.uniform_matrix_4_f32_slice(Some(ubicacion), false, &vista_proy.m);
                }

                for malla in self.mallas.values() {
                    malla.dibujar(gl);
                }

                // Mira fija en el centro de la pantalla, sin profundidad.
                gl.disable(glow::DEPTH_TEST);
                if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                    gl.uniform_matrix_4_f32_slice(
                        Some(ubicacion),
                        false,
                        &Mat4::identidad().m,
                    );
                }
                self.mira.dibujar(gl);
                gl.enable(glow::DEPTH_TEST);
            }
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
