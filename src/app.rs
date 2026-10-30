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
use crate::world::block::Bloque;
use crate::world::chunk::LADO_I;
use crate::world::{Mundo, raycast};

/// Velocidad de la camara libre, en unidades por segundo.
const VELOCIDAD: f32 = 7.0;
/// Velocidad con Shift pulsado.
const VELOCIDAD_RAPIDA: f32 = 20.0;
/// Alcance del rayo para romper y colocar bloques, en bloques.
const ALCANCE: f32 = 6.0;

pub struct App {
    gfx: Option<Gfx>,
    clock: Clock,
    input: Input,
    camara: Camera,
    mundo: Mundo,
    /// Chunk en el que esta la camara, para saber cuando recargar.
    centro: Option<(i32, i32)>,
    /// Tiempo acumulado desde el ultimo cambio de titulo.
    tiempo_titulo: f32,
}

impl App {
    pub fn new() -> Self {
        Self {
            gfx: None,
            clock: Clock::new(),
            input: Input::new(),
            camara: Camera::new(Vec3::new(8.5, 18.0, 30.0)),
            mundo: Mundo::new(2),
            centro: None,
            tiempo_titulo: 0.0,
        }
    }

    /// Carga y descarga chunks alrededor de la camara.
    ///
    /// Solo hace trabajo cuando la camara cambia de chunk, asi que se
    /// puede llamar en cada paso de simulacion.
    fn sincronizar_chunks(&mut self) {
        let p = self.camara.posicion;
        let centro = Mundo::clave_de(p.x.floor() as i32, p.z.floor() as i32);
        if self.centro == Some(centro) {
            return;
        }
        self.centro = Some(centro);

        let (nuevos, quitados) = self.mundo.ajustar(centro);
        let mallas: Vec<_> = nuevos
            .iter()
            .map(|clave| (*clave, self.mundo.malla_de(*clave)))
            .collect();

        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };
        for clave in quitados {
            gfx.quitar_chunk(clave);
        }
        for (clave, datos) in mallas {
            if let Err(err) = gfx.subir_chunk(clave, &datos) {
                eprintln!("Montworld: no se pudo subir el chunk {clave:?}: {err}");
            }
        }
    }

    /// Rehace la malla de un chunk concreto (si esta cargado).
    fn refrescar_chunk(&mut self, clave: (i32, i32)) {
        if !self.mundo.tiene_chunk(clave) {
            return;
        }
        let datos = self.mundo.malla_de(clave);
        if let Some(gfx) = self.gfx.as_mut()
            && let Err(err) = gfx.subir_chunk(clave, &datos)
        {
            eprintln!("Montworld: no se pudo actualizar el chunk {clave:?}: {err}");
        }
    }

    /// Rehace el chunk afectado por un cambio de bloque y, si el bloque
    /// estaba en un borde, tambien el vecino correspondiente.
    fn refrescar_alrededor(&mut self, x: i32, z: i32) {
        let (cx, cz) = Mundo::clave_de(x, z);
        self.refrescar_chunk((cx, cz));

        let lx = x.rem_euclid(LADO_I);
        let lz = z.rem_euclid(LADO_I);
        if lx == 0 {
            self.refrescar_chunk((cx - 1, cz));
        }
        if lx == LADO_I - 1 {
            self.refrescar_chunk((cx + 1, cz));
        }
        if lz == 0 {
            self.refrescar_chunk((cx, cz - 1));
        }
        if lz == LADO_I - 1 {
            self.refrescar_chunk((cx, cz + 1));
        }
    }

    /// Quita el bloque al que apunta la mira.
    fn romper_bloque(&mut self) {
        let Some(impacto) = raycast::lanzar(
            self.camara.posicion,
            self.camara.adelante(),
            ALCANCE,
            |x, y, z| self.mundo.bloque(x, y, z),
        ) else {
            return;
        };

        let [x, y, z] = impacto.bloque;
        self.mundo.poner(x, y, z, Bloque::Aire);
        self.refrescar_alrededor(x, z);
    }

    /// Coloca un bloque en la cara apuntada.
    fn colocar_bloque(&mut self) {
        let Some(impacto) = raycast::lanzar(
            self.camara.posicion,
            self.camara.adelante(),
            ALCANCE,
            |x, y, z| self.mundo.bloque(x, y, z),
        ) else {
            return;
        };

        // Si el rayo nace dentro de un bloque no hay cara donde colocar.
        if impacto.normal == [0, 0, 0] {
            return;
        }

        let [x, y, z] = raycast::vecino(&impacto);

        // No colocar un bloque encima del jugador.
        let p = self.camara.posicion;
        let ocupada = p.x.floor() as i32 == x
            && p.y.floor() as i32 == y
            && p.z.floor() as i32 == z;
        if ocupada {
            return;
        }

        if self.mundo.bloque(x, y, z).es_aire() && self.mundo.poner(x, y, z, Bloque::Piedra) {
            self.refrescar_alrededor(x, z);
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

        self.sincronizar_chunks();
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
                "Montworld - {fps} FPS - ({:.1}, {:.1}, {:.1}) - {} chunks",
                p.x,
                p.y,
                p.z,
                self.mundo.cantidad()
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
                // Vista inicial: el mundo desde fuera y algo elevada.
                self.camara.mirar_a(Vec3::new(8.0, 8.0, 8.0));
                self.centro = None;
                self.sincronizar_chunks();
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
                button,
                ..
            } => {
                if !self.input.capturado {
                    self.capturar_raton();
                } else {
                    match button {
                        MouseButton::Left => self.romper_bloque(),
                        MouseButton::Right => self.colocar_bloque(),
                        _ => {}
                    }
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
