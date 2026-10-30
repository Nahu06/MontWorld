//! Capa de aplicación: ventana, eventos de winit y bucle principal.
//!
//! Es la única capa que conoce la plataforma. Traduce eventos a acciones
//! del juego y mantiene sincronizadas las mallas de los chunks con el
//! estado de la simulación.

use std::path::PathBuf;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, WindowId};

use crate::base::clock::{Clock, PASO};
use crate::entrada::Input;
use crate::juego::Juego;
use crate::render::Render;
use crate::world::guardado;

pub struct App {
    render: Option<Render>,
    clock: Clock,
    entrada: Input,
    juego: Juego,
    ruta_partida: PathBuf,
    /// Tiempo acumulado desde el último cambio de título.
    tiempo_titulo: f32,
    /// Tiempo acumulado desde el último autoguardado.
    tiempo_guardado: f32,
}

impl App {
    pub fn nuevo() -> Self {
        Self {
            render: None,
            clock: Clock::new(),
            entrada: Input::new(),
            juego: Juego::nuevo(),
            ruta_partida: guardado::ruta_por_defecto(),
            tiempo_titulo: 0.0,
            tiempo_guardado: 0.0,
        }
    }

    /// Escribe la partida en disco (posición, cámara y bloques cambiados).
    fn guardar_partida(&self) {
        let partida = self.juego.partida();
        match guardado::guardar(&self.ruta_partida, &partida) {
            Ok(()) => println!(
                "Montworld: partida guardada en {} ({} cambios)",
                self.ruta_partida.display(),
                partida.cambios.len()
            ),
            Err(err) => eprintln!("Montworld: no se pudo guardar la partida: {err}"),
        }
    }

    /// Sube a la GPU las mallas nuevas y las que quedaron sucias.
    fn aplicar_cambios(&mut self) {
        let cambios = self.juego.tomar_cambios();
        let sucios = self.juego.tomar_sucios();

        let Some(render) = self.render.as_mut() else {
            return;
        };

        for clave in cambios.quitados {
            render.quitar_chunk(clave);
        }

        for clave in cambios.nuevos.into_iter().chain(sucios) {
            if !self.juego.mundo.tiene_chunk(clave) {
                continue;
            }
            let datos = self.juego.mundo.malla_de(clave);
            if let Err(err) = render.subir_chunk(clave, &datos) {
                eprintln!("Montworld: no se pudo subir el chunk {clave:?}: {err}");
            }
        }
    }

    /// Captura el ratón para mirar con él. Si el sistema no permite
    /// bloquearlo, se conforma con confinarlo a la ventana.
    fn capturar_raton(&mut self) {
        if let Some(render) = self.render.as_ref() {
            let ventana = render.ventana();
            let _ = ventana
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| ventana.set_cursor_grab(CursorGrabMode::Confined));
            ventana.set_cursor_visible(false);
        }
        self.entrada.capturado = true;
        self.entrada.soltar_todo();
        self.centrar_cursor();
    }

    /// Devuelve el cursor al centro de la ventana.
    ///
    /// En X11 no se puede bloquear el puntero de verdad: con solo
    /// confinarlo, al llegar al borde la cámara dejaría de girar.
    /// Recentrar el cursor en cada movimiento da giro infinito.
    fn centrar_cursor(&self) {
        if !self.entrada.capturado {
            return;
        }
        if let Some(render) = self.render.as_ref() {
            let ventana = render.ventana();
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
        if let Some(render) = self.render.as_ref() {
            let ventana = render.ventana();
            let _ = ventana.set_cursor_grab(CursorGrabMode::None);
            ventana.set_cursor_visible(true);
        }
        self.entrada.capturado = false;
        self.entrada.soltar_todo();
    }

    fn frame(&mut self) {
        let frame = self.clock.begin_frame();

        for _ in 0..frame.ticks {
            self.juego.actualizar(&mut self.entrada, PASO);
        }

        self.aplicar_cambios();

        // Autoguardado periódico, además del guardado al salir.
        self.tiempo_guardado += frame.delta;
        if self.tiempo_guardado >= 20.0 {
            self.tiempo_guardado = 0.0;
            self.guardar_partida();
        }

        let Some(render) = self.render.as_mut() else {
            return;
        };
        render.dibujar(&self.juego.jugador.camara);

        let fps = frame.fps.round() as u32;
        self.tiempo_titulo += frame.delta;
        if fps > 0 && self.tiempo_titulo >= 0.25 {
            self.tiempo_titulo = 0.0;
            let p = self.juego.jugador.camara.posicion;
            render.ventana().set_title(&format!(
                "Montworld - {fps} FPS - ({:.1}, {:.1}, {:.1}) - {} chunks",
                p.x,
                p.y,
                p.z,
                self.juego.mundo.cantidad()
            ));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.render.is_some() {
            return;
        }

        match Render::nuevo(event_loop) {
            Ok(render) => {
                self.render = Some(render);
                self.clock.reset();

                // Recuperar la partida anterior, si existe.
                match guardado::cargar(&self.ruta_partida) {
                    Ok(partida) => {
                        println!(
                            "Montworld: partida cargada de {} ({} cambios)",
                            self.ruta_partida.display(),
                            partida.cambios.len()
                        );
                        self.juego.cargar(partida);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        eprintln!("Montworld: no se pudo cargar la partida: {err}");
                    }
                }

                self.juego.reiniciar_streaming();
                self.aplicar_cambios();
            }
            Err(err) => {
                eprintln!("Montworld: no se pudo crear la ventana o el contexto GL: {err}");
                event_loop.exit();
            }
        }
    }

    /// En Android el sistema puede quitarnos la superficie gráfica al
    /// pasar a segundo plano. Se libera todo y `resumed` lo recrea.
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.render = None;
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
                if !self.entrada.capturado {
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
                            if tecla.logical_key == Key::Named(NamedKey::Escape) {
                                // Primero libera el ratón; si ya estaba
                                // libre, cierra el juego.
                                if self.entrada.capturado {
                                    self.liberar_raton();
                                } else {
                                    event_loop.exit();
                                }
                            } else {
                                self.entrada.presionar(codigo);
                            }
                        }
                        ElementState::Released => self.entrada.soltar(codigo),
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if !self.entrada.capturado {
                    self.capturar_raton();
                } else {
                    let cambio = match button {
                        MouseButton::Left => self.juego.romper(),
                        MouseButton::Right => self.juego.colocar(),
                        _ => false,
                    };
                    if cambio {
                        self.aplicar_cambios();
                    }
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(render) = self.render.as_mut() {
                    render.redimensionar(size.width, size.height);
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
            && self.entrada.capturado
        {
            self.entrada.sumar_raton(delta.0 as f32, delta.1 as f32);
            self.centrar_cursor();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.guardar_partida();
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Pide el siguiente frame. El vsync frena el bucle, así que no hay
        // espera activa gastando CPU.
        if let Some(render) = self.render.as_ref() {
            render.ventana().request_redraw();
        }
    }
}
