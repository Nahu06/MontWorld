//! Guardado y carga de la partida.
//!
//! El terreno se genera por procedimientos, así que no hace falta guardar
//! todos los bloques: basta con guardar los que el jugador modificó, más
//! la posición y la orientación de la cámara.
//!
//! Formato binario propio (little-endian), sin dependencias externas:
//!
//!   "MONT"            4 bytes de firma
//!   versión           u32
//!   posición          f32 × 3   (pies del jugador)
//!   yaw               f32
//!   pitch             f32
//!   cantidad          u32
//!   cambios           cantidad × (x i32, y i32, z i32, bloque u8)

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::block::Bloque;

const FIRMA: &[u8; 4] = b"MONT";
/// Versión 2 añade la hora del mundo. La 1 sigue siendo legible.
const VERSION: u32 = 2;
const VERSION_MINIMA: u32 = 1;

/// Hora por defecto al cargar partidas antiguas (media mañana).
pub const HORA_INICIAL: f32 = 0.15;

/// Cambios de bloques respecto al terreno generado, por coordenada global.
pub type Cambios = HashMap<(i32, i32, i32), Bloque>;

#[derive(Clone, Debug, Default)]
pub struct Partida {
    pub posicion: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// Hora del mundo en [0, 1): 0 amanecer, 0.25 mediodía,
    /// 0.5 atardecer, 0.75 medianoche.
    pub hora: f32,
    pub cambios: Cambios,
}

/// Ruta del archivo de partida en el sistema.
///
/// Usa `XDG_DATA_HOME` si existe; si no, `~/.local/share/`.
pub fn ruta_por_defecto() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|inicio| PathBuf::from(inicio).join(".local/share"))
        })
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("montworld").join("partida.mnt")
}

pub fn guardar(ruta: &Path, partida: &Partida) -> io::Result<()> {
    let mut datos = Vec::with_capacity(32 + partida.cambios.len() * 13);

    datos.extend_from_slice(FIRMA);
    datos.extend_from_slice(&VERSION.to_le_bytes());
    for valor in partida.posicion {
        datos.extend_from_slice(&valor.to_le_bytes());
    }
    datos.extend_from_slice(&partida.yaw.to_le_bytes());
    datos.extend_from_slice(&partida.pitch.to_le_bytes());
    datos.extend_from_slice(&partida.hora.to_le_bytes());

    let cantidad = u32::try_from(partida.cambios.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "demasiados cambios"))?;
    datos.extend_from_slice(&cantidad.to_le_bytes());

    for ((x, y, z), bloque) in &partida.cambios {
        datos.extend_from_slice(&x.to_le_bytes());
        datos.extend_from_slice(&y.to_le_bytes());
        datos.extend_from_slice(&z.to_le_bytes());
        datos.push(*bloque as u8);
    }

    if let Some(carpeta) = ruta.parent() {
        fs::create_dir_all(carpeta)?;
    }
    // Escritura atómica: se escribe a un temporal y se renombra, para no
    // dejar una partida a medias si el proceso muere.
    let temporal = ruta.with_extension("mnt.tmp");
    fs::write(&temporal, &datos)?;
    fs::rename(&temporal, ruta)
}

pub fn cargar(ruta: &Path) -> io::Result<Partida> {
    let datos = fs::read(ruta)?;
    let mut lector = Lector { datos: &datos, pos: 0 };

    if lector.bytes(4)? != FIRMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "el archivo no es una partida de Montworld",
        ));
    }

    let version = lector.u32()?;
    if !(VERSION_MINIMA..=VERSION).contains(&version) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("versión de partida no soportada: {version}"),
        ));
    }

    let posicion = [lector.f32()?, lector.f32()?, lector.f32()?];
    let yaw = lector.f32()?;
    let pitch = lector.f32()?;
    let hora = if version >= 2 {
        lector.f32()?
    } else {
        HORA_INICIAL
    };

    let cantidad = lector.u32()? as usize;
    let mut cambios = Cambios::with_capacity(cantidad);
    for _ in 0..cantidad {
        let x = lector.i32()?;
        let y = lector.i32()?;
        let z = lector.i32()?;
        let byte = lector.bytes(1)?[0];
        cambios.insert((x, y, z), Bloque::desde_byte(byte));
    }

    Ok(Partida {
        posicion,
        yaw,
        pitch,
        hora,
        cambios,
    })
}

struct Lector<'a> {
    datos: &'a [u8],
    pos: usize,
}

impl<'a> Lector<'a> {
    fn bytes(&mut self, cantidad: usize) -> io::Result<&'a [u8]> {
        let fin = self.pos + cantidad;
        if fin > self.datos.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "partida incompleta",
            ));
        }
        let trozo = &self.datos[self.pos..fin];
        self.pos = fin;
        Ok(trozo)
    }

    fn u32(&mut self) -> io::Result<u32> {
        let bytes: [u8; 4] = self.bytes(4)?.try_into().unwrap();
        Ok(u32::from_le_bytes(bytes))
    }

    fn i32(&mut self) -> io::Result<i32> {
        let bytes: [u8; 4] = self.bytes(4)?.try_into().unwrap();
        Ok(i32::from_le_bytes(bytes))
    }

    fn f32(&mut self) -> io::Result<f32> {
        let bytes: [u8; 4] = self.bytes(4)?.try_into().unwrap();
        Ok(f32::from_le_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ruta_temporal(nombre: &str) -> PathBuf {
        let mut ruta = std::env::temp_dir();
        ruta.push(format!("montworld-test-{}-{nombre}.mnt", std::process::id()));
        ruta
    }

    #[test]
    fn guardar_y_cargar_conserva_todo() {
        let ruta = ruta_temporal("ida-vuelta");
        let mut partida = Partida {
            posicion: [1.5, 22.25, -3.0],
            yaw: 0.75,
            pitch: -0.4,
            hora: 0.42,
            cambios: Cambios::new(),
        };
        partida.cambios.insert((1, 2, 3), Bloque::Piedra);
        partida.cambios.insert((-4, 16, 20), Bloque::Aire);
        partida.cambios.insert((0, 0, 0), Bloque::Arena);

        guardar(&ruta, &partida).expect("guardar");
        let leida = cargar(&ruta).expect("cargar");
        let _ = fs::remove_file(&ruta);

        assert_eq!(leida.posicion, partida.posicion);
        assert_eq!(leida.yaw, partida.yaw);
        assert_eq!(leida.pitch, partida.pitch);
        assert_eq!(leida.hora, partida.hora);
        assert_eq!(leida.cambios, partida.cambios);
    }

    #[test]
    fn una_partida_de_la_version_1_usa_la_hora_inicial() {
        let ruta = ruta_temporal("v1");
        let mut datos = Vec::new();
        datos.extend_from_slice(FIRMA);
        datos.extend_from_slice(&1u32.to_le_bytes());
        for valor in [1.0f32, 20.0, 2.0, 0.5, -0.2] {
            datos.extend_from_slice(&valor.to_le_bytes());
        }
        datos.extend_from_slice(&0u32.to_le_bytes());
        fs::write(&ruta, &datos).expect("escribir");

        let leida = cargar(&ruta).expect("cargar versión 1");
        let _ = fs::remove_file(&ruta);

        assert_eq!(leida.hora, HORA_INICIAL);
        assert_eq!(leida.posicion, [1.0, 20.0, 2.0]);
    }

    #[test]
    fn un_archivo_ajeno_falla_con_error_claro() {
        let ruta = ruta_temporal("basura");
        fs::write(&ruta, b"NO ES UNA PARTIDA").expect("escribir");
        let error = cargar(&ruta).expect_err("debería fallar");
        let _ = fs::remove_file(&ruta);
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn una_partida_truncada_falla() {
        let ruta = ruta_temporal("truncada");
        let partida = Partida::default();
        guardar(&ruta, &partida).expect("guardar");

        let mut datos = fs::read(&ruta).expect("leer");
        datos.truncate(datos.len() - 2);
        fs::write(&ruta, &datos).expect("escribir");

        let error = cargar(&ruta).expect_err("debería fallar");
        let _ = fs::remove_file(&ruta);
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
