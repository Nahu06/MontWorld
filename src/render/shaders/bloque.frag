// Shader del mundo: color plano por vértice.
// La cabecera #version (y la precisión en GLES) la añade shader.rs.

in vec3 v_color;

out vec4 f_color;

void main() {
    f_color = vec4(v_color, 1.0);
}
