//! Malla en la GPU: un VAO con posición y color entrelazados.
//!
//! Es la unidad de dibujo de una capa concreta del mundo (un chunk). Se
//! sube una vez y se dibuja muchas veces.

use std::error::Error;

use glow::HasContext;

/// Flotantes por vértice: posición (3) + color (3).
pub const FLOATS_POR_VERTICE: i32 = 6;

pub struct Malla {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    vertices: i32,
}

impl Malla {
    pub fn subir(gl: &glow::Context, datos: &[f32]) -> Result<Self, Box<dyn Error>> {
        unsafe {
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, por_bytes(datos), glow::STATIC_DRAW);

            let paso = FLOATS_POR_VERTICE * 4;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, paso, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, paso, 12);

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            Ok(Self {
                vao,
                vbo,
                vertices: (datos.len() / FLOATS_POR_VERTICE as usize) as i32,
            })
        }
    }

    pub fn dibujar(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, self.vertices);
            gl.bind_vertex_array(None);
        }
    }

    pub fn borrar(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
        }
    }
}

/// Convierte una lista de flotantes en bytes para subirla a la GPU.
fn por_bytes(datos: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(datos.as_ptr().cast(), datos.len() * 4) }
}
