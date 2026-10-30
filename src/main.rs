//! Punto de entrada de Montworld.
//!
//! Crea el bucle de eventos del sistema y se lo entrega a la aplicacion,
//! que se encarga de la ventana, el contexto grafico y el bucle principal.

mod app;
mod camera;
mod clock;
mod gfx;
mod input;
mod math;
mod world;

use std::error::Error;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;

    // El ritmo lo marca el vsync al intercambiar buffers; no hay que
    // dormir el hilo principal para esperar eventos.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
