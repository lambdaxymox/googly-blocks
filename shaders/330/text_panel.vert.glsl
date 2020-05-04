#version 330 core

layout (location = 0) in vec2 v_pos;
layout (location = 1) in vec2 v_tex;
out vec2 tex_coord;


void main() {
    tex_coord = v_tex;
    gl_Position = vec4(v_pos, 0.0, 1.0);
}
