//! Reloj del juego.
//!
//! Separa la simulacion del renderizado: la simulacion avanza en pasos
//! fijos de 1/60 s y el render se hace una vez por frame. Asi el juego se
//! comporta igual a 60 fps que a 144 fps, o cuando hay tirones.

use std::time::{Duration, Instant};

/// Duracion de un paso logico. 60 pasos por segundo.
const TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// El mismo paso logico, en segundos, para la fisica y el movimiento.
pub const PASO: f32 = 1.0 / 60.0;

/// Si el juego se queda bloqueado mucho tiempo (por ejemplo al mover la
/// ventana), no se acumulan cientos de pasos: se descarta el atraso.
const MAX_TICKS: u32 = 5;

pub struct Frame {
    /// Pasos logicos que hay que ejecutar en este frame.
    pub ticks: u32,
    /// Delta de tiempo real del frame, en segundos.
    pub delta: f32,
    /// FPS medidos con una media movil.
    pub fps: f32,
}

/// Ventana de medida de FPS: se cuentan frames reales durante este
/// tiempo. Así el contador refleja los tirones, no una media engañosa.
const VENTANA_FPS: Duration = Duration::from_millis(500);

pub struct Clock {
    last: Instant,
    accumulator: Duration,
    fps: f32,
    frames_ventana: u32,
    tiempo_ventana: Duration,
}

impl Clock {
    pub fn new() -> Self {
        Self {
            last: Instant::now(),
            accumulator: Duration::ZERO,
            fps: 0.0,
            frames_ventana: 0,
            tiempo_ventana: Duration::ZERO,
        }
    }

    /// La primera vez que se llama tras crear la ventana (o al volver de
    /// segundo plano) reinicia la medida para no contar el tiempo perdido.
    pub fn reset(&mut self) {
        self.last = Instant::now();
        self.accumulator = Duration::ZERO;
        self.fps = 0.0;
        self.frames_ventana = 0;
        self.tiempo_ventana = Duration::ZERO;
    }

    pub fn begin_frame(&mut self) -> Frame {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last);
        self.last = now;

        let delta_f32 = delta.as_secs_f32();

        // FPS reales: frames contados divididos por el tiempo medido.
        self.frames_ventana += 1;
        self.tiempo_ventana += delta;
        if self.tiempo_ventana >= VENTANA_FPS {
            self.fps = self.frames_ventana as f32 / self.tiempo_ventana.as_secs_f32();
            self.frames_ventana = 0;
            self.tiempo_ventana = Duration::ZERO;
        }

        self.accumulator += delta;
        let mut ticks = (self.accumulator.as_nanos() / TICK.as_nanos()) as u32;
        if ticks > MAX_TICKS {
            ticks = MAX_TICKS;
            self.accumulator = Duration::ZERO;
        } else {
            self.accumulator -= TICK * ticks;
        }

        Frame {
            ticks,
            delta: delta_f32,
            fps: self.fps,
        }
    }
}
