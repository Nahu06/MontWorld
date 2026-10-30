//! Capa grafica: ventana, contexto de OpenGL y dibujo de la escena.
//!
//! En escritorio se pide OpenGL 3.3 core; en Android, OpenGL ES 3.0. Los
//! shaders estan escritos una vez para cada version y se elige segun la
//! plataforma.
//!
//! La geometria que se dibuja es la malla de un chunk, subida a la GPU como
//! un unico buffer de posicion + color.

use std::error::Error;
use std::num::NonZeroU32;

use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext, Version,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow, finalize_window};
use glow::HasContext;
use raw_window_handle::HasWindowHandle;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::camera::Camera;
use crate::math::Mat4;

const ANCHO_INICIAL: u32 = 1280;
const ALTO_INICIAL: u32 = 720;

/// Flotantes por vertice: posicion (3) + color (3).
const FLOATS_POR_VERTICE: i32 = 6;

#[cfg(not(target_os = "android"))]
const VERTEX_DESKTOP: &str = r#"#version 330 core
layout(location = 0) in vec3 a_posicion;
layout(location = 1) in vec3 a_color;
uniform mat4 u_vista_proy;
out vec3 v_color;
void main() {
    v_color = a_color;
    gl_Position = u_vista_proy * vec4(a_posicion, 1.0);
}
"#;

#[cfg(not(target_os = "android"))]
const FRAGMENT_DESKTOP: &str = r#"#version 330 core
in vec3 v_color;
out vec4 f_color;
void main() {
    f_color = vec4(v_color, 1.0);
}
"#;

#[cfg(target_os = "android")]
const VERTEX_ANDROID: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec3 a_posicion;
layout(location = 1) in vec3 a_color;
uniform mat4 u_vista_proy;
out vec3 v_color;
void main() {
    v_color = a_color;
    gl_Position = u_vista_proy * vec4(a_posicion, 1.0);
}
"#;

#[cfg(target_os = "android")]
const FRAGMENT_ANDROID: &str = r#"#version 300 es
precision mediump float;
in vec3 v_color;
out vec4 f_color;
void main() {
    f_color = vec4(v_color, 1.0);
}
"#;

pub struct Gfx {
    gl: glow::Context,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    window: Window,

    programa: glow::Program,
    u_vista_proy: Option<glow::UniformLocation>,
    malla: Option<Malla>,
    mira: Malla,
    ancho: u32,
    alto: u32,
}

/// Geometria en la GPU: un VAO con posicion y color entrelazados.
struct Malla {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    vertices: i32,
}

impl Malla {
    fn subir(gl: &glow::Context, datos: &[f32]) -> Result<Self, Box<dyn Error>> {
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

    fn dibujar(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, self.vertices);
            gl.bind_vertex_array(None);
        }
    }

    fn borrar(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
        }
    }
}

impl Gfx {
    pub fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn Error>> {
        // Buffer de profundidad desde el principio: el mundo 3D lo necesita.
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_depth_size(24)
            .with_stencil_size(8)
            .with_transparency(false);

        // La configuracion se pide antes de crear la ventana porque en
        // Android la ventana todavia no existe en este punto.
        let (_, gl_config) = DisplayBuilder::new()
            .build(event_loop, template, |configuraciones| {
                configuraciones
                    .reduce(|mejor, candidata| {
                        if candidata.num_samples() > mejor.num_samples() {
                            candidata
                        } else {
                            mejor
                        }
                    })
                    .expect("no hay ninguna configuracion de OpenGL disponible")
            })
            .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;

        let window = finalize_window(event_loop, atributos_ventana(), &gl_config)?;

        let context = crear_contexto(&window, &gl_config)?;
        let surface = crear_superficie(&window, &gl_config)?;
        let context = context.make_current(&surface)?;

        let _ = surface.set_swap_interval(
            &context,
            SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
        );

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|nombre| {
                gl_config.display().get_proc_address(nombre).cast()
            })
        };

        let version = unsafe { gl.get_parameter_string(glow::VERSION) };
        let renderer = unsafe { gl.get_parameter_string(glow::RENDERER) };
        println!("Montworld: GL {version} | {renderer}");

        let programa = crear_programa(&gl)?;
        let mira = Malla::subir(&gl, &vertices_mira())?;
        let tamano = window.inner_size();

        Ok(Self {
            gl,
            surface,
            context,
            window,
            programa,
            u_vista_proy: None,
            malla: None,
            mira,
            ancho: tamano.width.max(1),
            alto: tamano.height.max(1),
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn aspecto(&self) -> f32 {
        self.ancho as f32 / self.alto.max(1) as f32
    }

    pub fn resize(&mut self, ancho: u32, alto: u32) {
        if let (Some(w), Some(h)) = (NonZeroU32::new(ancho), NonZeroU32::new(alto)) {
            self.surface.resize(&self.context, w, h);
            self.ancho = ancho;
            self.alto = alto;
        }
    }

    /// Sube una malla nueva a la GPU, sustituyendo la anterior.
    pub fn cargar_malla(&mut self, datos: &[f32]) -> Result<(), Box<dyn Error>> {
        let nueva = Malla::subir(&self.gl, datos)?;
        if let Some(anterior) = self.malla.replace(nueva) {
            anterior.borrar(&self.gl);
        }
        println!("Montworld: malla con {} vertices", datos.len() / 6);
        Ok(())
    }

    pub fn render(&mut self, camara: &Camera) {
        let vista_proy = camara.vista_proyeccion(self.aspecto());

        unsafe {
            self.gl.viewport(0, 0, self.ancho as i32, self.alto as i32);
            self.gl.clear_color(0.53, 0.72, 0.92, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            if self.malla.is_none() {
                let _ = self.surface.swap_buffers(&self.context);
                return;
            }

            self.gl.enable(glow::DEPTH_TEST);
            self.gl.use_program(Some(self.programa));

            if self.u_vista_proy.is_none() {
                self.u_vista_proy = self.gl.get_uniform_location(self.programa, "u_vista_proy");
            }
            if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(ubicacion), false, &vista_proy.m);
            }

            if let Some(malla) = self.malla.as_ref() {
                malla.dibujar(&self.gl);
            }

            // Mira fija en el centro de la pantalla, sin profundidad.
            self.gl.disable(glow::DEPTH_TEST);
            if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(ubicacion), false, &Mat4::identidad().m);
            }
            self.mira.dibujar(&self.gl);
            self.gl.enable(glow::DEPTH_TEST);
        }

        let _ = self.surface.swap_buffers(&self.context);
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        if let Some(malla) = self.malla.take() {
            malla.borrar(&self.gl);
        }
        self.mira.borrar(&self.gl);
        unsafe {
            self.gl.delete_program(self.programa);
        }
    }
}

fn atributos_ventana() -> winit::window::WindowAttributes {
    let atributos = Window::default_attributes().with_title("Montworld");

    // En Android la ventana ya ocupa toda la pantalla.
    #[cfg(not(target_os = "android"))]
    let atributos = atributos.with_inner_size(winit::dpi::LogicalSize::new(
        ANCHO_INICIAL as f64,
        ALTO_INICIAL as f64,
    ));

    atributos
}

fn crear_contexto(
    window: &Window,
    gl_config: &Config,
) -> Result<glutin::context::NotCurrentContext, Box<dyn Error>> {
    let raw_handle = window.window_handle()?.as_raw();

    #[cfg(target_os = "android")]
    let peticion = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::Gles(Some(Version::new(3, 0))));

    #[cfg(not(target_os = "android"))]
    let peticion = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))));

    let atributos = peticion.build(Some(raw_handle));

    // Si el driver no puede dar la version pedida, se prueba un contexto
    // por defecto y se informa de la version real al arrancar.
    let context = unsafe { gl_config.display().create_context(gl_config, &atributos) }
        .or_else(|_| unsafe {
            let por_defecto = ContextAttributesBuilder::new().build(Some(raw_handle));
            gl_config.display().create_context(gl_config, &por_defecto)
        })?;

    Ok(context)
}

fn crear_superficie(
    window: &Window,
    gl_config: &Config,
) -> Result<Surface<WindowSurface>, Box<dyn Error>> {
    let atributos = window.build_surface_attributes(Default::default())?;
    let surface = unsafe { gl_config.display().create_window_surface(gl_config, &atributos)? };
    Ok(surface)
}

fn crear_programa(gl: &glow::Context) -> Result<glow::Program, Box<dyn Error>> {
    #[cfg(target_os = "android")]
    let (fuente_vertice, fuente_fragmento) = (VERTEX_ANDROID, FRAGMENT_ANDROID);
    #[cfg(not(target_os = "android"))]
    let (fuente_vertice, fuente_fragmento) = (VERTEX_DESKTOP, FRAGMENT_DESKTOP);

    unsafe {
        let vertice = compilar_shader(gl, glow::VERTEX_SHADER, fuente_vertice, "vertice")?;
        let fragmento = compilar_shader(gl, glow::FRAGMENT_SHADER, fuente_fragmento, "fragmento")?;

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

unsafe fn compilar_shader(
    gl: &glow::Context,
    tipo: u32,
    fuente: &str,
    nombre: &str,
) -> Result<glow::Shader, Box<dyn Error>> {
    unsafe {
        let shader = gl.create_shader(tipo)?;
        gl.shader_source(shader, fuente);
        gl.compile_shader(shader);

        if !gl.get_shader_compile_status(shader) {
            let error = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("error al compilar el shader de {nombre}: {error}").into());
        }

        Ok(shader)
    }
}

/// Convierte una lista de flotantes en bytes para subirla a la GPU.
fn por_bytes(datos: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(datos.as_ptr().cast(), datos.len() * 4) }
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
