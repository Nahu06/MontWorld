//! Capa de aplicación: ventana, eventos de winit y bucle principal.
//!
//! Es la única capa que conoce la plataforma. Traduce eventos a acciones
//! del juego y mantiene sincronizadas las mallas de los chunks con el
//! estado de la simulación.

use std::path::PathBuf;

use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
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
    /// Hay que recentrar el cursor en el próximo frame.
    recentrar: bool,
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
            recentrar: false,
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

    /// Reconstruye la barra de bloques con la selección actual.
    fn actualizar_barra(&mut self) {
        if let Some(render) = self.render.as_mut() {
            let colores = self.juego.colores_barra();
            if let Err(err) = render.actualizar_barra(&colores, self.juego.seleccion()) {
                eprintln!("Montworld: no se pudo actualizar la barra: {err}");
            }
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
            // Los chunks vacíos (solo aire) no gastan memoria en la GPU.
            if datos.is_empty() {
                render.quitar_chunk(clave);
                continue;
            }
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

        // Recentrar el cursor una vez por frame (no en cada evento de
        // ratón): en X11 cada warp es una ida y vuelta al servidor.
        if self.recentrar {
            self.centrar_cursor();
            self.recentrar = false;
        }

        // Autoguardado periódico, además del guardado al salir.
        self.tiempo_guardado += frame.delta;
        if self.tiempo_guardado >= 20.0 {
            self.tiempo_guardado = 0.0;
            self.guardar_partida();
        }

        let Some(render) = self.render.as_mut() else {
            return;
        };
        render.dibujar(&self.juego.jugador.camara, self.juego.hora());

        let fps = frame.fps.round() as u32;
        self.tiempo_titulo += frame.delta;
        if fps > 0 && self.tiempo_titulo >= 0.25 {
            self.tiempo_titulo = 0.0;
            let p = self.juego.jugador.camara.posicion;
            render.ventana().set_title(&format!(
                "Montworld - {fps} FPS - ({:.1}, {:.1}, {:.1}) - {} chunks - {} - {}",
                p.x,
                p.y,
                p.z,
                self.juego.mundo.cantidad(),
                hora_legible(self.juego.hora()),
                crate::world::geografia::nombre(self.juego.zona())
            ));
        }
    }
}

/// Convierte la hora del mundo (0..1) en un reloj HH:MM.
fn hora_legible(hora: f32) -> String {
    let minutos = (hora.rem_euclid(1.0) * 24.0 * 60.0) as u32;
    format!("{:02}:{:02}", (minutos / 60) % 24, minutos % 60)
}

/// Teclas 1 a 6: índice de la barra rápida.
fn indice_de_barra(codigo: KeyCode) -> Option<usize> {
    match codigo {
        KeyCode::Digit1 => Some(0),
        KeyCode::Digit2 => Some(1),
        KeyCode::Digit3 => Some(2),
        KeyCode::Digit4 => Some(3),
        KeyCode::Digit5 => Some(4),
        KeyCode::Digit6 => Some(5),
        _ => None,
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

                self.aplicar_cambios();
                self.actualizar_barra();
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
                            } else if let Some(indice) = indice_de_barra(codigo) {
                                self.juego.seleccionar(indice);
                                self.actualizar_barra();
                            } else if codigo == KeyCode::KeyT {
                                // Atajo de prueba: adelanta una hora.
                                self.juego.avanzar_hora(1.0 / 24.0);
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

            WindowEvent::MouseWheel { delta, .. } if self.entrada.capturado => {
                let desplazamiento = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if desplazamiento != 0.0 {
                    self.juego.ciclar_seleccion(desplazamiento < 0.0);
                    self.actualizar_barra();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(render) = self.render.as_mut() {
                    render.redimensionar(size.width, size.height);
                }
                self.actualizar_barra();
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
            self.recentrar = true;
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
