#version 420 core

//in vec2 v_board;
uniform vec4 u_frag_color;
out vec4 frag_color;


void main() {
    frag_color = u_frag_color;
}
