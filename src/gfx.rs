//! Capa grafica: ventana, contexto de OpenGL y presentacion de frames.
//!
//! En escritorio se pide OpenGL 3.3 core; en Android, OpenGL ES 3.0. El
//! codigo de render posterior debe tener en cuenta esa diferencia al
//! escribir shaders (330 core frente a 300 es).

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

/// Ancho y alto iniciales de la ventana en escritorio.
const WINDOW_WIDTH: f64 = 1280.0;
const WINDOW_HEIGHT: f64 = 720.0;

pub struct Gfx {
    gl: glow::Context,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    window: Window,
    tiempo: f32,
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

        // Sincroniza con el refresco de la pantalla. Si el driver no lo
        // permite, se sigue sin vsync en vez de abortar.
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

        Ok(Self {
            gl,
            surface,
            context,
            window,
            tiempo: 0.0,
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&self, ancho: u32, alto: u32) {
        if let (Some(w), Some(h)) = (NonZeroU32::new(ancho), NonZeroU32::new(alto)) {
            self.surface.resize(&self.context, w, h);
        }
    }

    /// Dibuja un frame. De momento solo limpia la pantalla con un color
    /// que cambia despacio: sirve para comprobar que el bucle corre.
    pub fn render(&mut self, delta: f32) {
        self.tiempo += delta;
        let fase = self.tiempo * 0.6;
        let r = 0.05 + 0.03 * fase.sin();
        let g = 0.08 + 0.03 * (fase + 2.094).sin();
        let b = 0.14 + 0.03 * (fase + 4.188).sin();

        unsafe {
            self.gl.clear_color(r, g, b, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        let _ = self.surface.swap_buffers(&self.context);
    }
}

fn atributos_ventana() -> winit::window::WindowAttributes {
    let atributos = Window::default_attributes().with_title("Montworld");

    // En Android la ventana ya ocupa toda la pantalla.
    #[cfg(not(target_os = "android"))]
    let atributos = atributos.with_inner_size(winit::dpi::LogicalSize::new(
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
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
