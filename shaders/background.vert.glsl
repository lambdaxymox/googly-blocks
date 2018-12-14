#version 420 core

in vec3 v_pos;
//uniform mat4 proj_mat, view_mat, model_mat;
//out vec2 v_board;


void main() {
    //v_board = vec2 (v_pos);
    //gl_Position = proj_mat * view_mat * model_mat * vec4 (v_pos, 1.0);
    gl_Position = vec4 (v_pos, 1.0);
}
