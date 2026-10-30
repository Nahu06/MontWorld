//! Generación procedural del terreno.
//!
//! Todo depende de coordenadas globales, de modo que dos chunks vecinos
//! encajan sin costuras ni saltos de altura.

/// Altura de la superficie en coordenadas globales. Va de 3 a 13 para que
/// quepa de sobra en los 16 bloques de alto de un chunk.
pub fn altura(x: i32, z: i32) -> i32 {
    let x = x as f32;
    let z = z as f32;
    let valor = 8.0 + ruido_suave(x, z, 26.0) * 3.0 + ruido_suave(x, z, 9.0) * 1.2;
    valor.round().clamp(3.0, 13.0) as i32
}

/// Ruido de valor con interpolación suave. Determinista: solo depende de
/// las coordenadas.
fn ruido_suave(x: f32, z: f32, escala: f32) -> f32 {
    let fx = x / escala;
    let fz = z / escala;

    let x0 = fx.floor();
    let z0 = fz.floor();
    let tx = suavizar(fx - x0);
    let tz = suavizar(fz - z0);

    let (xi, zi) = (x0 as i32, z0 as i32);
    let a = ruido_entero(xi, zi);
    let b = ruido_entero(xi + 1, zi);
    let c = ruido_entero(xi, zi + 1);
    let d = ruido_entero(xi + 1, zi + 1);

    let arriba = a + (b - a) * tx;
    let abajo = c + (d - c) * tx;
    arriba + (abajo - arriba) * tz
}

/// Curva de suavizado: 3t² - 2t³.
fn suavizar(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Valor pseudoaleatorio en [-1, 1] para una celda de la rejilla de ruido.
fn ruido_entero(x: i32, z: i32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(z.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    let unidad = ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0;
    unidad * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_altura_se_mantiene_en_rango() {
        for z in -80..80 {
            for x in -80..80 {
                let h = altura(x, z);
                assert!((3..=13).contains(&h), "altura {h} en ({x}, {z})");
            }
        }
    }

    #[test]
    fn la_altura_es_determinista() {
        assert_eq!(altura(12, -34), altura(12, -34));
    }

    #[test]
    fn la_altura_cambia_de_forma_suave() {
        // Columnas vecinas no pueden saltar más de un par de bloques.
        for z in -40..40 {
            for x in -40..40 {
                let diferencia = (altura(x, z) - altura(x + 1, z)).abs();
                assert!(diferencia <= 2, "salto de {diferencia} en ({x}, {z})");
            }
        }
    }
}
