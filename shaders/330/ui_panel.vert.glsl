#version 330 core

uniform Matrices {
    mat4 m_trans;
    mat4 m_gui_scale;
};

layout (location = 0) in vec2 v_pos;
layout (location = 1) in vec2 v_tex;

out Data {
    out vec2 tex_coord;
} DataOut;

void main() {
    DataOut.tex_coord = v_tex;
    gl_Position = m_trans * m_gui_scale * vec4 (v_pos, 0.0, 1.0);
}
