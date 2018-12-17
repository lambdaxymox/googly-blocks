#version 420 core

uniform Matrices {
    mat4 m_proj;
    mat4 m_view;
    mat4 m_model;
};

in vec3 v_pos;
in vec2 v_tex;
out vec2 tex_coord;

void main() {
    tex_coord = v_tex;
    gl_Position = m_proj * m_view * m_model * vec4 (v_pos, 1.0);
}
