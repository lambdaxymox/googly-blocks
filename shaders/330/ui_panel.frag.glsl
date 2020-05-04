#version 330 core

in Data {
    in vec2 tex_coord;
} DataIn;

uniform sampler2D board_tex;

out vec4 frag_color;

void main() {
    frag_color = texture (board_tex, DataIn.tex_coord);
}
