//! Ciclo de vida de la aplicacion y bucle principal.

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, WindowId};

use crate::camera::Camera;
use crate::clock::{Clock, PASO};
use crate::gfx::Gfx;
use crate::input::Input;
use crate::math::Vec3;
use crate::world::Chunk;
use crate::world::meshing;

/// Velocidad de la camara libre, en unidades por segundo.
const VELOCIDAD: f32 = 7.0;
/// Velocidad con Shift pulsado.
const VELOCIDAD_RAPIDA: f32 = 20.0;

pub struct App {
    gfx: Option<Gfx>,
    clock: Clock,
    input: Input,
    camara: Camera,
    mundo: Chunk,
    /// Tiempo acumulado desde el ultimo cambio de titulo.
    tiempo_titulo: f32,
}

impl App {
    pub fn new() -> Self {
        let mut mundo = Chunk::new();
        mundo.generar_terreno();

        Self {
            gfx: None,
            clock: Clock::new(),
            input: Input::new(),
            camara: Camera::new(Vec3::new(8.0, 22.0, 34.0)),
            mundo,
            tiempo_titulo: 0.0,
        }
    }

    /// Genera la malla del mundo y la sube a la GPU. Se llama cada vez que
    /// se crea (o se vuelve a crear) el contexto grafico.
    fn subir_mundo(&mut self) {
        let vertices = meshing::malla(&self.mundo);
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        if let Err(err) = gfx.cargar_malla(&vertices) {
            eprintln!("Montworld: no se pudo subir la malla del mundo: {err}");
        }
    }

    /// Captura el raton para mirar con el. Si el sistema no permite
    /// bloquearlo, se conforma con confinarlo a la ventana.
    fn capturar_raton(&mut self) {
        if let Some(gfx) = self.gfx.as_ref() {
            let ventana = gfx.window();
            let _ = ventana
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| ventana.set_cursor_grab(CursorGrabMode::Confined));
            ventana.set_cursor_visible(false);
        }
        self.input.capturado = true;
        self.input.soltar_todo();
        self.centrar_cursor();
    }

    /// Devuelve el cursor al centro de la ventana.
    ///
    /// En X11 no se puede bloquear el puntero de verdad: con solo
    /// confinarlo, al llegar al borde la camara dejaria de girar. Recentrar
    /// el cursor en cada movimiento da la sensacion de giro infinito.
    fn centrar_cursor(&self) {
        if !self.input.capturado {
            return;
        }
        if let Some(gfx) = self.gfx.as_ref() {
            let ventana = gfx.window();
            let tamano = ventana.inner_size();
            if tamano.width > 0 && tamano.height > 0 {
                let _ = ventana.set_cursor_position(winit::dpi::PhysicalPosition::new(
                    (tamano.width / 2) as i32,
                    (tamano.height / 2) as i32,
                ));
            }
        }
    }

    fn liberar_raton(&mut self) {
        if let Some(gfx) = self.gfx.as_ref() {
            let ventana = gfx.window();
            let _ = ventana.set_cursor_grab(CursorGrabMode::None);
            ventana.set_cursor_visible(true);
        }
        self.input.capturado = false;
        self.input.soltar_todo();
    }

    /// Un paso logico de simulacion (1/60 s de juego).
    fn update(&mut self) {
        let (dx, dy) = self.input.tomar_raton();
        if dx != 0.0 || dy != 0.0 {
            self.camara.girar(dx, dy);
        }

        let mut direccion = Vec3::ZERO;
        if self.input.abajo(KeyCode::KeyW) {
            direccion = direccion + self.camara.adelante_horizontal();
        }
        if self.input.abajo(KeyCode::KeyS) {
            direccion = direccion - self.camara.adelante_horizontal();
        }
        if self.input.abajo(KeyCode::KeyD) {
            direccion = direccion + self.camara.derecha();
        }
        if self.input.abajo(KeyCode::KeyA) {
            direccion = direccion - self.camara.derecha();
        }
        if self.input.abajo(KeyCode::Space) {
            direccion.y += 1.0;
        }
        if self.input.abajo(KeyCode::ControlLeft) || self.input.abajo(KeyCode::ControlRight) {
            direccion.y -= 1.0;
        }

        if direccion.length() > 0.0 {
            let rapida = self.input.abajo(KeyCode::ShiftLeft)
                || self.input.abajo(KeyCode::ShiftRight);
            let velocidad = if rapida { VELOCIDAD_RAPIDA } else { VELOCIDAD };
            self.camara
                .mover(direccion.normalize() * (velocidad * PASO));
        }
    }

    fn frame(&mut self) {
        let frame = self.clock.begin_frame();

        for _ in 0..frame.ticks {
            self.update();
        }

        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        gfx.render(&self.camara);

        let fps = frame.fps.round() as u32;
        self.tiempo_titulo += frame.delta;
        if fps > 0 && self.tiempo_titulo >= 0.25 {
            self.tiempo_titulo = 0.0;
            let p = self.camara.posicion;
            gfx.window().set_title(&format!(
                "Montworld - {fps} FPS - ({:.1}, {:.1}, {:.1})",
                p.x, p.y, p.z
            ));
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
                self.subir_mundo();
                // Vista inicial: el chunk entero desde fuera.
                self.camara.mirar_a(Vec3::new(8.0, 8.0, 8.0));
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

            WindowEvent::Focused(true) => {
                if !self.input.capturado {
                    self.capturar_raton();
                }
            }
            WindowEvent::Focused(false) => self.liberar_raton(),

            WindowEvent::KeyboardInput {
                event: tecla,
                is_synthetic: false,
                ..
            } => {
                if let PhysicalKey::Code(codigo) = tecla.physical_key {
                    match tecla.state {
                        ElementState::Pressed => {
                            let escape = tecla.logical_key == Key::Named(NamedKey::Escape);
                            if escape {
                                // Primero libera el raton; si ya estaba
                                // libre, cierra el juego.
                                if self.input.capturado {
                                    self.liberar_raton();
                                } else {
                                    event_loop.exit();
                                }
                            } else {
                                self.input.presionar(codigo);
                            }
                        }
                        ElementState::Released => self.input.soltar(codigo),
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.input.capturado {
                    self.capturar_raton();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(gfx) = self.gfx.as_mut() {
                    gfx.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.input.capturado
        {
            self.input.sumar_raton(delta.0 as f32, delta.1 as f32);
            self.centrar_cursor();
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
