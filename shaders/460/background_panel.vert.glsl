#version 460 core

in vec3 v_pos;
in vec2 v_tex;
uniform mat4 m_gui_scale;
out vec2 tex_coord;


void main() {
    tex_coord = v_tex;
    gl_Position = m_gui_scale * vec4 (v_pos, 1.0);
}