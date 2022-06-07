#version 330 core

in vec2 tex_coord;
uniform sampler2D tex;
uniform vec4 text_color;
out vec4 frag_color;


void main() {
    frag_color = text_color * texture(tex, tex_coord);
}
