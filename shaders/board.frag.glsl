#version 420 core

in vec2 tex_coord;
uniform sampler2D board_tex;
out vec4 frag_color;

void main() {
    frag_color = texture (board_tex, tex_coord);
}
