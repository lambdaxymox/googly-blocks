#version 460 core

in vec2 v_pos;
in vec2 v_tex;
out vec2 st;


void main() {
    st = v_tex;
    gl_Position = vec4(v_pos, 0.0, 1.0);
}
