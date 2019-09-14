#version 460 core

uniform Matrices {
    mat4 m_trans;
    mat4 m_gui_scale;
};

in vec3 v_pos;
in vec2 v_tex;

out Data {
    out vec2 tex_coord;
} DataOut;

void main() {
    DataOut.tex_coord = v_tex;
    gl_Position = m_trans * m_gui_scale * vec4 (v_pos, 1.0);
}
