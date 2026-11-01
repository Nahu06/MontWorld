//! Generación procedural del terreno.
//!
//! Todo depende de coordenadas globales, de modo que dos chunks vecinos
//! encajan sin costuras ni saltos de altura.

/// Altura de la superficie en coordenadas globales.
///
/// Puede ser negativa (valles y simas) y superar los 40 bloques (montañas).
/// No hay tope de mundo: solo se limita el relieve para que sea razonable.
pub fn altura(x: i32, z: i32) -> i32 {
    let x = x as f32;
    let z = z as f32;
    let valor = 6.0
        + ruido_suave(x, z, 60.0) * 16.0
        + ruido_suave(x, z, 24.0) * 6.0
        + ruido_suave(x, z, 8.0) * 2.0;
    valor.round().clamp(-20.0, 48.0) as i32
}

/// Indica si una celda subterránea debe quedar hueca (cueva).
///
/// Usa ruido 3D estirado en vertical, así que forma galerías alargadas
/// en vez de burbujas redondas.
pub fn cueva(x: i32, y: i32, z: i32) -> bool {
    let valor = ruido_suave_3d(x as f32, y as f32 * 1.7, z as f32, 15.0);
    valor > 0.42
}

/// Indica si en esa columna nace un árbol. Determinista por coordenada.
pub fn hay_arbol(x: i32, z: i32) -> bool {
    let azar = (ruido_entero(x, z) + 1.0) * 0.5;
    azar < 0.02
}

/// Altura del tronco del árbol de esa columna (4 o 5 bloques).
pub fn altura_arbol(x: i32, z: i32) -> i32 {
    let azar = (ruido_entero(x.wrapping_add(7), z.wrapping_sub(3)) + 1.0) * 0.5;
    4 + (azar > 0.5) as i32
}

/// Ruido de valor 3D con interpolación suave.
fn ruido_suave_3d(x: f32, y: f32, z: f32, escala: f32) -> f32 {
    let fx = x / escala;
    let fy = y / escala;
    let fz = z / escala;

    let x0 = fx.floor();
    let y0 = fy.floor();
    let z0 = fz.floor();
    let tx = suavizar(fx - x0);
    let ty = suavizar(fy - y0);
    let tz = suavizar(fz - z0);

    let (xi, yi, zi) = (x0 as i32, y0 as i32, z0 as i32);

    let mut valor = 0.0;
    for dz in 0..=1 {
        for dy in 0..=1 {
            for dx in 0..=1 {
                let muestra = ruido_entero_3d(xi + dx, yi + dy, zi + dz);
                let peso = (if dx == 1 { tx } else { 1.0 - tx })
                    * (if dy == 1 { ty } else { 1.0 - ty })
                    * (if dz == 1 { tz } else { 1.0 - tz });
                valor += muestra * peso;
            }
        }
    }
    valor
}

fn ruido_entero_3d(x: i32, y: i32, z: i32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(z.wrapping_mul(2_147_483_647));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    let unidad = ((h ^ (h >> 16)) & 0xffff) as f32 / 65_535.0;
    unidad * 2.0 - 1.0
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
                assert!((-20..=48).contains(&h), "altura {h} en ({x}, {z})");
            }
        }
    }

    #[test]
    fn hay_alturas_positivas_y_negativas() {
        let mut minimo = i32::MAX;
        let mut maximo = i32::MIN;
        for z in -200..200 {
            for x in -200..200 {
                let h = altura(x, z);
                minimo = minimo.min(h);
                maximo = maximo.max(h);
            }
        }
        assert!(minimo < 0, "debería haber valles bajo cero");
        assert!(maximo > 20, "debería haber montañas altas");
    }

    #[test]
    fn la_altura_es_determinista() {
        assert_eq!(altura(12, -34), altura(12, -34));
    }

    #[test]
    fn las_cuevas_aparecen_bajo_tierra_pero_no_en_el_fondo() {
        let mut huecos = 0;
        let mut total = 0;
        for z in -20..20 {
            for x in -20..20 {
                for y in 2..10 {
                    total += 1;
                    if cueva(x, y, z) {
                        huecos += 1;
                    }
                }
            }
        }
        assert!(huecos > 0, "debería haber cuevas");
        assert!(huecos < total / 2, "las cuevas no deberían comerse el mundo");

        // También hay cuevas en profundidad (mundo infinito hacia abajo).
        let profundas = (-120..-80)
            .flat_map(|y| (0..20).map(move |x| (x, y)))
            .filter(|(x, y)| cueva(*x, *y, 0))
            .count();
        assert!(profundas > 0, "debería haber cuevas profundas");
    }

    #[test]
    fn los_arboles_son_deterministas() {
        let mut encontrados = 0;
        for z in 0..64 {
            for x in 0..64 {
                if hay_arbol(x, z) {
                    encontrados += 1;
                    assert!((4..=5).contains(&altura_arbol(x, z)));
                }
            }
        }
        assert!(encontrados > 0, "debería haber árboles");
    }

    #[test]
    fn la_altura_cambia_de_forma_suave() {
        // Columnas vecinas no pueden saltar más de un par de bloques.
        for z in -40..40 {
            for x in -40..40 {
                let diferencia = (altura(x, z) - altura(x + 1, z)).abs();
                assert!(diferencia <= 3, "salto de {diferencia} en ({x}, {z})");
            }
        }
    }
}
