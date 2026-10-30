//! Ciclo de vida de la aplicacion y bucle principal.

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

use crate::clock::Clock;
use crate::gfx::Gfx;

pub struct App {
    gfx: Option<Gfx>,
    clock: Clock,
    /// Ultimo valor entero de FPS puesto en el titulo, para no reescribirlo
    /// en cada frame.
    fps_mostrados: u32,
}

impl App {
    pub fn new() -> Self {
        Self {
            gfx: None,
            clock: Clock::new(),
            fps_mostrados: 0,
        }
    }

    /// Un paso logico de simulacion (1/60 s de juego).
    fn update(&mut self) {
        // Todavia no hay entidades: aqui iran el jugador, el mundo y la
        // fisica.
    }

    fn frame(&mut self) {
        let frame = self.clock.begin_frame();

        for _ in 0..frame.ticks {
            self.update();
        }

        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        gfx.render(frame.delta);

        let fps = frame.fps.round() as u32;
        if fps > 0 && fps != self.fps_mostrados {
            self.fps_mostrados = fps;
            gfx.window().set_title(&format!("Montworld - {fps} FPS"));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }

        match Gfx::new(event_loop) {
            Ok(gfx) => {
                self.gfx = Some(gfx);
                self.clock.reset();
            }
            Err(err) => {
                eprintln!("Montworld: no se pudo crear la ventana o el contexto GL: {err}");
                event_loop.exit();
            }
        }
    }

    /// En Android el sistema puede quitarnos la superficie grafica al pasar
    /// a segundo plano. Se libera todo y `resumed` lo vuelve a crear.
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.gfx = None;
        self.clock.reset();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(gfx) = self.gfx.as_ref() {
                    gfx.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Pide el siguiente frame. El vsync frena el bucle, asi que no hay
        // espera activa gastando CPU.
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window().request_redraw();
        }
    }
}
