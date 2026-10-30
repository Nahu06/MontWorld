//! Compilación y enlace de shaders.
//!
//! Los shaders viven en `render/shaders/` sin la línea `#version`, que se
//! añade aquí según la plataforma. Así el mismo archivo sirve para
//! escritorio (GLSL 330) y Android (GLSL ES 300).

use std::error::Error;

use glow::HasContext;

/// Prefijo del shader de vértices.
fn cabecera_vertice() -> &'static str {
    #[cfg(target_os = "android")]
    {
        "#version 300 es\n"
    }
    #[cfg(not(target_os = "android"))]
    {
        "#version 330 core\n"
    }
}

/// Prefijo del shader de fragmentos (en GLES la precisión es obligatoria).
fn cabecera_fragmento() -> &'static str {
    #[cfg(target_os = "android")]
    {
        "#version 300 es\nprecision mediump float;\n"
    }
    #[cfg(not(target_os = "android"))]
    {
        "#version 330 core\n"
    }
}

pub fn programa(
    gl: &glow::Context,
    vertice: &str,
    fragmento: &str,
) -> Result<glow::Program, Box<dyn Error>> {
    unsafe {
        let vertice = compilar(
            gl,
            glow::VERTEX_SHADER,
            cabecera_vertice(),
            vertice,
            "vértice",
        )?;
        let fragmento = compilar(
            gl,
            glow::FRAGMENT_SHADER,
            cabecera_fragmento(),
            fragmento,
            "fragmento",
        )?;

        let programa = gl.create_program()?;
        gl.attach_shader(programa, vertice);
        gl.attach_shader(programa, fragmento);
        gl.link_program(programa);

        let enlazado = gl.get_program_link_status(programa);
        gl.detach_shader(programa, vertice);
        gl.detach_shader(programa, fragmento);
        gl.delete_shader(vertice);
        gl.delete_shader(fragmento);

        if !enlazado {
            let error = gl.get_program_info_log(programa);
            gl.delete_program(programa);
            return Err(format!("no se pudo enlazar el programa GL: {error}").into());
        }

        Ok(programa)
    }
}

unsafe fn compilar(
    gl: &glow::Context,
    tipo: u32,
    cabecera: &str,
    cuerpo: &str,
    nombre: &str,
) -> Result<glow::Shader, Box<dyn Error>> {
    unsafe {
        let shader = gl.create_shader(tipo)?;
        gl.shader_source(shader, &format!("{cabecera}{cuerpo}"));
        gl.compile_shader(shader);

        if !gl.get_shader_compile_status(shader) {
            let error = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("error al compilar el shader de {nombre}: {error}").into());
        }

        Ok(shader)
    }
}
