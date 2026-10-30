//! Punto de entrada de Montworld.
//!
//! Crea el bucle de eventos del sistema y se lo entrega a la capa de
//! aplicación, que orquesta juego y render.

mod app;
mod base;
mod entrada;
mod juego;
mod jugador;
mod render;
mod world;

use std::error::Error;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;

    // El ritmo lo marca el vsync al intercambiar buffers; no hay que
    // dormir el hilo principal para esperar eventos.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::App::nuevo();
    event_loop.run_app(&mut app)?;

    Ok(())
}
