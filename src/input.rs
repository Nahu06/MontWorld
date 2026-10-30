//! Estado de entrada: teclas pulsadas y movimiento del raton.
//!
//! No depende de winit mas alla de los codigos de tecla. La captura del
//! cursor se gestiona desde `app`, porque necesita la ventana.

use std::collections::HashSet;

use winit::keyboard::KeyCode;

pub struct Input {
    teclas: HashSet<KeyCode>,
    raton_dx: f32,
    raton_dy: f32,
    pub capturado: bool,
}

impl Input {
    pub fn new() -> Self {
        Self {
            teclas: HashSet::new(),
            raton_dx: 0.0,
            raton_dy: 0.0,
            capturado: false,
        }
    }

    pub fn presionar(&mut self, tecla: KeyCode) {
        self.teclas.insert(tecla);
    }

    pub fn soltar(&mut self, tecla: KeyCode) {
        self.teclas.remove(&tecla);
    }

    pub fn soltar_todo(&mut self) {
        self.teclas.clear();
    }

    pub fn abajo(&self, tecla: KeyCode) -> bool {
        self.teclas.contains(&tecla)
    }

    pub fn sumar_raton(&mut self, dx: f32, dy: f32) {
        self.raton_dx += dx;
        self.raton_dy += dy;
    }

    /// Devuelve el movimiento acumulado del raton y lo deja a cero.
    pub fn tomar_raton(&mut self) -> (f32, f32) {
        let delta = (self.raton_dx, self.raton_dy);
        self.raton_dx = 0.0;
        self.raton_dy = 0.0;
        delta
    }
}
