//! Geografía del mundo: zonas por altitud.
//!
//! El mundo no tiene tope de altura ni de profundidad, así que las capas
//! se definen por la altura absoluta (y). Este módulo es el punto único
//! para decidir qué corresponde a cada nivel: hoy sirve para mostrar la
//! zona y más adelante para repartir minerales y objetos por profundidad.

/// Capa geográfica del mundo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zona {
    /// Por encima de las montañas: solo aire y construcciones.
    Cielo,
    /// Cumbres altas.
    Montana,
    /// Nivel de superficie habitual.
    Superficie,
    /// Primeras capas bajo tierra.
    Subsuelo,
    /// Zona profunda, con cuevas grandes.
    Profundo,
    /// Fondo sin límite conocido.
    Abismo,
}

/// Altitud a la que empieza cada zona.
pub const INICIO_MONTANA: i32 = 32;
pub const INICIO_CIELO: i32 = 64;
pub const INICIO_SUBSUELO: i32 = 0;
pub const INICIO_PROFUNDO: i32 = -32;
pub const INICIO_ABISMO: i32 = -128;

pub fn zona(y: i32) -> Zona {
    match y {
        y if y >= INICIO_CIELO => Zona::Cielo,
        y if y >= INICIO_MONTANA => Zona::Montana,
        y if y >= INICIO_SUBSUELO => Zona::Superficie,
        y if y > INICIO_PROFUNDO => Zona::Subsuelo,
        y if y >= INICIO_ABISMO => Zona::Profundo,
        _ => Zona::Abismo,
    }
}

pub fn nombre(zona: Zona) -> &'static str {
    match zona {
        Zona::Cielo => "cielo",
        Zona::Montana => "montaña",
        Zona::Superficie => "superficie",
        Zona::Subsuelo => "subsuelo",
        Zona::Profundo => "profundo",
        Zona::Abismo => "abismo",
    }
}

/// Profundidad en bloques bajo la superficie habitual (positiva hacia
/// abajo). Se usará para repartir minerales y objetos por profundidad.
#[allow(dead_code)]
pub fn profundidad(y: i32) -> i32 {
    INICIO_SUBSUELO - y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_zona_empieza_donde_corresponde() {
        assert_eq!(zona(100), Zona::Cielo);
        assert_eq!(zona(INICIO_CIELO), Zona::Cielo);
        assert_eq!(zona(INICIO_MONTANA), Zona::Montana);
        assert_eq!(zona(INICIO_SUBSUELO), Zona::Superficie);
        assert_eq!(zona(-1), Zona::Subsuelo);
        assert_eq!(zona(INICIO_PROFUNDO), Zona::Profundo);
        assert_eq!(zona(INICIO_ABISMO), Zona::Profundo);
        assert_eq!(zona(INICIO_ABISMO - 1), Zona::Abismo);
    }

    #[test]
    fn la_zona_baja_al_bajar() {
        let orden = [
            Zona::Cielo,
            Zona::Montana,
            Zona::Superficie,
            Zona::Subsuelo,
            Zona::Profundo,
            Zona::Abismo,
        ];
        for pareja in orden.windows(2) {
            assert_ne!(pareja[0], pareja[1]);
        }
        assert_eq!(zona(10), Zona::Superficie);
        assert_eq!(zona(-200), Zona::Abismo);
    }

    #[test]
    fn la_profundidad_crece_hacia_abajo() {
        assert!(profundidad(-50) > profundidad(0));
        assert!(profundidad(20) < 0);
    }
}
