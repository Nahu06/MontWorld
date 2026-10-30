// Shader del mundo: posición + color.
// La cabecera #version la añade shader.rs según la plataforma.

layout(location = 0) in vec3 a_posicion;
layout(location = 1) in vec3 a_color;

uniform mat4 u_vista_proy;

out vec3 v_color;

void main() {
    v_color = a_color;
    gl_Position = u_vista_proy * vec4(a_posicion, 1.0);
}
