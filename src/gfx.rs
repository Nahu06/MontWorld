//! Capa grafica: ventana, contexto de OpenGL y dibujo de la escena.
//!
//! En escritorio se pide OpenGL 3.3 core; en Android, OpenGL ES 3.0. Los
//! shaders se escriben una vez y se elige la version segun la plataforma.
//!
//! La escena actual es solo una referencia visual (cubos de colores) para
//! poder comprobar el movimiento de la camara. Se sustituira por el mundo
//! de bloques.

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
use crate::math::Vec3;

const ANCHO_INICIAL: u32 = 1280;
const ALTO_INICIAL: u32 = 720;

/// Cuantos flotantes ocupa un vertice: posicion (3) + color (3).
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
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    vertices: i32,
    ancho: u32,
    alto: u32,
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

        // Escena de referencia: el suelo y unos cubos de colores.
        let datos = escena_de_prueba();
        let vertices = (datos.len() / FLOATS_POR_VERTICE as usize) as i32;

        let (vao, vbo) = unsafe {
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;

            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                por_bytes(&datos),
                glow::STATIC_DRAW,
            );

            let paso = FLOATS_POR_VERTICE * 4;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, paso, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, paso, 12);

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            (vao, vbo)
        };

        let tamano = window.inner_size();

        Ok(Self {
            gl,
            surface,
            context,
            window,
            programa,
            u_vista_proy: None,
            vao,
            vbo,
            vertices,
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

    pub fn render(&mut self, camara: &Camera) {
        let vista_proy = camara.vista_proyeccion(self.aspecto());

        unsafe {
            self.gl.viewport(0, 0, self.ancho as i32, self.alto as i32);
            self.gl.clear_color(0.53, 0.72, 0.92, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

            self.gl.enable(glow::DEPTH_TEST);
            self.gl.use_program(Some(self.programa));

            if self.u_vista_proy.is_none() {
                self.u_vista_proy =
                    self.gl.get_uniform_location(self.programa, "u_vista_proy");
            }
            if let Some(ubicacion) = self.u_vista_proy.as_ref() {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(ubicacion), false, &vista_proy.m);
            }

            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLES, 0, self.vertices);
            self.gl.bind_vertex_array(None);
        }

        let _ = self.surface.swap_buffers(&self.context);
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_program(self.programa);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_buffer(self.vbo);
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

/// Cubo con caras sombreadas para distinguir el volumen sin iluminacion.
fn empujar_cubo(salida: &mut Vec<f32>, centro: Vec3, lado: f32, color: Vec3) {
    // Esquinas del cubo unidad: (0,0,0) ... (1,1,1).
    const ESQUINAS: [[f32; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];

    // Caras como cuadruples de esquinas, y su brillo relativo.
    const CARAS: [[usize; 4]; 6] = [
        [4, 5, 6, 7], // frente (+Z)
        [1, 0, 3, 2], // atras  (-Z)
        [0, 4, 7, 3], // izquierda (-X)
        [5, 1, 2, 6], // derecha (+X)
        [3, 7, 6, 2], // arriba (+Y)
        [0, 1, 5, 4], // abajo  (-Y)
    ];
    const BRILLO: [f32; 6] = [0.92, 0.72, 0.72, 0.86, 1.0, 0.55];

    let mitad = lado * 0.5;
    let mut vertice = |indice: usize, brillo: f32| {
        let e = ESQUINAS[indice];
        let posicion = Vec3::new(
            centro.x + (e[0] * 2.0 - 1.0) * mitad,
            centro.y + (e[1] * 2.0 - 1.0) * mitad,
            centro.z + (e[2] * 2.0 - 1.0) * mitad,
        );
        salida.extend_from_slice(&[
            posicion.x,
            posicion.y,
            posicion.z,
            color.x * brillo,
            color.y * brillo,
            color.z * brillo,
        ]);
    };

    for (indice, cara) in CARAS.iter().enumerate() {
        let brillo = BRILLO[indice];
        vertice(cara[0], brillo);
        vertice(cara[1], brillo);
        vertice(cara[2], brillo);
        vertice(cara[0], brillo);
        vertice(cara[2], brillo);
        vertice(cara[3], brillo);
    }
}

fn escena_de_prueba() -> Vec<f32> {
    let mut datos = Vec::new();

    // Suelo de cubos verdes.
    let verde = Vec3::new(0.36, 0.62, 0.28);
    let mut x = -6;
    while x <= 6 {
        let mut z = -6;
        while z <= 6 {
            empujar_cubo(&mut datos, Vec3::new(x as f32 * 2.0, -2.0, z as f32 * 2.0), 2.0, verde);
            z += 2;
        }
        x += 2;
    }

    // Pilares y cubos de colores para tener referencias en el espacio.
    let piedra = Vec3::new(0.62, 0.62, 0.66);
    let azul = Vec3::new(0.25, 0.45, 0.85);
    let rojo = Vec3::new(0.85, 0.30, 0.25);
    let amarillo = Vec3::new(0.90, 0.80, 0.25);

    empujar_cubo(&mut datos, Vec3::new(0.0, -1.0, 0.0), 1.0, piedra);
    empujar_cubo(&mut datos, Vec3::new(0.0, 0.0, 0.0), 1.0, piedra);
    empujar_cubo(&mut datos, Vec3::new(0.0, 1.0, 0.0), 1.0, azul);

    empujar_cubo(&mut datos, Vec3::new(-4.0, -0.5, -6.0), 2.0, rojo);
    empujar_cubo(&mut datos, Vec3::new(5.0, -0.25, -8.0), 2.5, amarillo);
    empujar_cubo(&mut datos, Vec3::new(-7.0, 0.0, 3.0), 2.0, azul);
    empujar_cubo(&mut datos, Vec3::new(8.0, 0.5, 2.0), 3.0, piedra);

    datos
}
