#version 460 core

uniform Matrices {
    mat4 m_proj;
    mat4 m_view;
    mat4 m_model;
};
uniform vec2 gui_scale;
in vec3 v_pos;
in vec2 v_tex;

out Data {
    out vec2 tex_coord;
} DataOut;

void main() {
    DataOut.tex_coord = v_tex;
    vec3 scale = vec3 (gui_scale, 0.0);
    gl_Position = m_proj * m_view * m_model * vec4 (scale * v_pos, 1.0);
}
