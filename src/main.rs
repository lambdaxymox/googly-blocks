/*
 *  Googly Blocks is a video game.
 *  Copyright (C) 2018,2019,2020  Christopher Blanchard
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
extern crate glfw;
extern crate bmfa;
extern crate cgmath;
extern crate toml;
extern crate log;
extern crate rand;
extern crate file_logger;
extern crate teximage2d;


mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[macro_use]
mod macros;

mod mesh;
mod gl_help;
mod playing_field;

use gl_help as glh;
use cgmath as math; 

use bmfa::BitmapFontAtlas;
use glfw::{Action, Context, Key};
use gl::types::{GLfloat, GLint, GLuint, GLvoid, GLsizeiptr};
use log::{info};
use math::{Array, One, Matrix4};
use mesh::ObjMesh;
use teximage2d::TexImage2D;
use playing_field::{
    BlockPosition, GooglyBlock, PlayingFieldState,
    GooglyBlockPiece, GooglyBlockRotation, GooglyBlockElement, GooglyBlockMove,
    LandedBlocksQuery
};
use rand::prelude as rng;
use rand::Rng;

use std::io;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::Duration;

// OpenGL extension constants.
const GL_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FE;
const GL_MAX_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FF;

// Green.
const TEXT_COLOR: [f32; 4] = [
    38_f32 / 255_f32, 239_f32 / 255_f32, 29_f32 / 255_f32, 255_f32 / 255_f32
];
// Default value for the color buffer.
const CLEAR_COLOR: [f32; 4] = [0.2_f32, 0.2_f32, 0.2_f32, 1.0_f32];
// Default value for the depth buffer.
const CLEAR_DEPTH: [f32; 4] = [1.0_f32, 1.0_f32, 1.0_f32, 1.0_f32];


/// Load texture image into the GPU.
fn send_to_gpu_texture(tex_data: &TexImage2D, wrapping_mode: GLuint) -> Result<GLuint, String> {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA as i32, tex_data.width as i32, tex_data.height as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE,
            tex_data.as_ptr() as *const GLvoid
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrapping_mode as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrapping_mode as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as GLint);
    }
    debug_assert!(tex > 0);

    let mut max_aniso = 0.0;
    unsafe {
        gl::GetFloatv(GL_MAX_TEXTURE_MAX_ANISOTROPY_EXT, &mut max_aniso);
        // Set the maximum!
        gl::TexParameterf(gl::TEXTURE_2D, GL_TEXTURE_MAX_ANISOTROPY_EXT, max_aniso);
    }

    Ok(tex)
}


#[derive(Copy, Clone)]
struct ShaderSource {
    vert_name: &'static str,
    vert_source: &'static str,
    frag_name: &'static str,
    frag_source: &'static str,
}

fn send_to_gpu_shaders(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    let mut vert_reader = io::Cursor::new(source.vert_source);
    let mut frag_reader = io::Cursor::new(source.frag_source);
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, source.vert_name,
        &mut frag_reader, source.frag_name
    ).unwrap();
    debug_assert!(sp > 0);

    sp
}

fn create_shaders_background() -> ShaderSource {
    let vert_source = include_shader!("background_panel.vert.glsl");
    let frag_source = include_shader!("background_panel.frag.glsl");

    ShaderSource { 
        vert_name: "background_panel.vert.glsl",
        vert_source: vert_source,
        frag_name: "background_panel.frag.glsl",
        frag_source: frag_source,
    }
}

#[inline]
fn send_to_gpu_shaders_background(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_geometry_background() -> ObjMesh {
    let points: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [-1.0, -1.0], [ 1.0, -1.0],
        [1.0, 1.0], [-1.0,  1.0], [-1.0, -1.0],
    ];
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0],
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
    ];

    ObjMesh::new(points, tex_coords)
}

#[derive(Copy, Clone)]
struct BackgroundPanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

fn create_buffers_geometry_background() -> BackgroundPanelHandle {
    let v_pos_loc = 0;
    let v_tex_loc = 1;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);
    
    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    BackgroundPanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }    
}

fn send_to_gpu_geometry_background(handle: BackgroundPanelHandle, mesh: &ObjMesh) {
    unsafe {
        // Load position data.
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        // Load the texture coordinates.
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );

        // Enable the arrays for use by the shader.
        gl::BindVertexArray(handle.vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::VertexAttribPointer(handle.v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(handle.v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(handle.v_pos_loc);
        gl::EnableVertexAttribArray(handle.v_tex_loc);
    }
}

fn create_textures_background() -> TexImage2D {
    let asset: &'static [u8; 27695] = include_asset!("title.png");
    teximage2d::load_from_memory(asset).unwrap().image
}

fn send_to_gpu_textures_background(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct BackgroundPanelUniforms { 
    gui_scale_x: f32,
    gui_scale_y: f32,
}

fn send_to_gpu_uniforms_background_panel(sp: GLuint, uniforms: BackgroundPanelUniforms) {
    let gui_scale_mat = Matrix4::from_nonuniform_scale(
        uniforms.gui_scale_x, uniforms.gui_scale_y, 0.0
    );
    let m_gui_scale_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_gui_scale").as_ptr())
    };
    debug_assert!(m_gui_scale_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(m_gui_scale_loc, 1, gl::FALSE, gui_scale_mat.as_ptr());
    }
}

#[derive(Copy, Clone)]
struct BackgroundPanelSpec { 
    height: usize, 
    width: usize, 
}

#[derive(Copy, Clone)]
struct GLBackgroundPanel {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

#[derive(Copy, Clone)]
struct BackgroundPanel {
    height: usize,
    width: usize,
    buffer: GLBackgroundPanel,
}

fn load_background_panel(game: &mut glh::GLState, spec: BackgroundPanelSpec) -> BackgroundPanel {
    let shader_source = create_shaders_background();
    let mesh = create_geometry_background();
    let tex_image = create_textures_background();
    let sp = send_to_gpu_shaders_background(game, shader_source);
    let handle = create_buffers_geometry_background();
    send_to_gpu_geometry_background(handle, &mesh);
    let tex = send_to_gpu_textures_background(&tex_image);
    let buffer = GLBackgroundPanel {
        sp: sp,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
        vao: handle.vao,
        tex: tex,
    };

    BackgroundPanel {
        buffer: buffer,
        height: spec.height,
        width: spec.width,
    }
}

fn update_uniforms_background_panel(game: &mut Game) {
    let panel_width = game.background.width as f32;
    let panel_height = game.background.height as f32;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = BackgroundPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_background_panel(game.background.buffer.sp, uniforms);
}


fn create_shaders_ui_panel() -> ShaderSource {
    let vert_source = include_shader!("ui_panel.vert.glsl");
    let frag_source = include_shader!("ui_panel.frag.glsl");

    ShaderSource { 
        vert_name: "ui_panel.vert.glsl",
        vert_source: vert_source, 
        frag_name: "ui_panel.frag.glsl",
        frag_source: frag_source 
    }
}

fn send_to_gpu_shaders_ui_panel(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_geometry_ui_panel() -> ObjMesh {
    let points: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [-1.0, -1.0], [ 1.0, -1.0],
        [1.0, 1.0], [-1.0,  1.0], [-1.0, -1.0]
    ];
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0],
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
    ];

    ObjMesh::new(points, tex_coords)
}

#[derive(Copy, Clone)]
struct UIPanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

fn create_buffers_geometry_ui_panel() -> UIPanelHandle {
    let v_pos_loc = 0;
    let v_tex_loc = 1;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    UIPanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_ui_panel(handle: UIPanelHandle, mesh: &ObjMesh) {
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );

        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );

        gl::BindVertexArray(handle.vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::VertexAttribPointer(handle.v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(handle.v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(handle.v_pos_loc);
        gl::EnableVertexAttribArray(handle.v_tex_loc);
    }
}

fn create_textures_ui_panel() -> TexImage2D {
    let asset: &'static [u8; 31235] = include_asset!("ui_panel.png");
    teximage2d::load_from_memory(asset).unwrap().image
}

fn send_to_gpu_textures_ui_panel(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct UIPanelSpec {
    height: usize,
    width: usize,
}

struct UIPanel {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
    height: usize,
    width: usize,
}


#[derive(Copy, Clone)]
struct UIPanelUniforms {
    gui_scale_x: f32,
    gui_scale_y: f32,
}

fn send_to_gpu_uniforms_ui_panel(sp: GLuint, uniforms: UIPanelUniforms) {
    let trans_mat = Matrix4::one();
    let gui_scale_mat = Matrix4::from_nonuniform_scale(uniforms.gui_scale_x, uniforms.gui_scale_y, 0.0);

    let ubo_index = unsafe {
        gl::GetUniformBlockIndex(sp, glh::gl_str("Matrices").as_ptr())
    };
    debug_assert!(ubo_index != gl::INVALID_INDEX);

    let mut ubo_size = 0;
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index, gl::UNIFORM_BLOCK_DATA_SIZE, &mut ubo_size
        );
    }
    debug_assert!(ubo_size > 0);

    let mut indices = [0; 2];
    let mut sizes = [0; 2];
    let mut offsets = [0; 2];
    let mut types = [0; 2];
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index,
            gl::UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES, indices.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 2, indices.as_ptr() as *const u32,
            gl::UNIFORM_OFFSET, offsets.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 2, indices.as_ptr() as *const u32,
            gl::UNIFORM_SIZE, sizes.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 2, indices.as_ptr() as *const u32,
            gl::UNIFORM_TYPE, types.as_mut_ptr()
        );
    }

    // Copy the uniform block data into a buffer to be passed to the GPU.
    let mut buffer = vec![0 as u8; ubo_size as usize];
    unsafe {
        ptr::copy(&trans_mat, mem::transmute(&mut buffer[offsets[1] as usize]), 1);
        ptr::copy(&gui_scale_mat, mem::transmute(&mut buffer[offsets[0] as usize]), 1);
    }

    let mut ubo = 0;
    unsafe {
        gl::GenBuffers(1, &mut ubo);
    }
    debug_assert!(ubo > 0);
    unsafe {
        gl::BindBuffer(gl::UNIFORM_BUFFER, ubo);
        gl::BufferData(
            gl::UNIFORM_BUFFER, ubo_size as GLsizeiptr,
            buffer.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindBufferBase(gl::UNIFORM_BUFFER, ubo_index, ubo);
    }
}

fn load_ui_panel(game: &mut glh::GLState, spec: UIPanelSpec, uniforms: UIPanelUniforms) -> UIPanel {
    let shader_source = create_shaders_ui_panel();
    let sp = send_to_gpu_shaders_ui_panel(game, shader_source);
    let mesh = create_geometry_ui_panel();
    let handle = create_buffers_geometry_ui_panel();
    send_to_gpu_geometry_ui_panel(handle, &mesh);
    let tex_image = create_textures_ui_panel();
    let tex = send_to_gpu_textures_ui_panel(&tex_image);
    send_to_gpu_uniforms_ui_panel(sp, uniforms);

    UIPanel {
        sp: sp,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
        vao: handle.vao,
        tex: tex,
        height: spec.height,
        width: spec.width,
    }
}

fn update_ui_panel_uniforms(game: &mut Game) {
    let panel_width = game.ui.ui_panel.width as f32;
    let panel_height = game.ui.ui_panel.height as f32;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_ui_panel(game.ui.ui_panel.sp, uniforms);
}


/// Create the shaders for the next panel in the game's user interface.
fn create_shaders_next_piece_panel() -> ShaderSource {
    let vert_source = include_shader!("next_piece_panel.vert.glsl");
    let frag_source = include_shader!("next_piece_panel.frag.glsl");

    ShaderSource { 
        vert_name: "next_piece_panel.vert.glsl",
        vert_source: vert_source,
        frag_name: "next_piece_panel.frag.glsl",
        frag_source: frag_source,
    }
}

struct PieceMeshes {
    t: ObjMesh,
    j: ObjMesh,
    z: ObjMesh,
    o: ObjMesh,
    s: ObjMesh,
    l: ObjMesh,
    i: ObjMesh,
}

fn create_geometry_t_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-0.5, 0.5], [0.0, 1.0], [-0.5, 1.0],
        [-0.5, 0.5], [0.0, 0.5], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 1.0], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 0.5], [ 0.5, 1.0],
        [ 0.0, 0.0], [0.5, 0.5], [ 0.0, 0.5],
        [ 0.0, 0.0], [0.5, 0.0], [ 0.5, 0.5],
        [ 0.5, 0.5], [1.0, 1.0], [ 0.5, 1.0],
        [ 0.5, 0.5], [1.0, 0.5], [ 1.0, 1.0],        
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 3_f32 / 3_f32],
        [0_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
    ];

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_j_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-0.5, 0.5], [0.0, 1.0], [-0.5, 1.0],
        [-0.5, 0.5], [0.0, 0.5], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 1.0], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 0.5], [ 0.5, 1.0],
        [ 0.5, 0.5], [1.0, 1.0], [ 0.5, 1.0],
        [ 0.5, 0.5], [1.0, 0.5], [ 1.0, 1.0],
        [ 0.5, 0.0], [1.0, 0.5], [ 0.5, 0.5],
        [ 0.5, 0.0], [1.0, 0.0], [ 1.0, 0.5],       
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
        [0_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
    ];
    
    ObjMesh::new(points, tex_coords)
}

fn create_geometry_z_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-0.5, 0.5], [0.0, 1.0], [-0.5, 1.0],
        [-0.5, 0.5], [0.0, 0.5], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 1.0], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 0.5], [ 0.5, 1.0],
        [ 0.0, 0.0], [0.5, 0.5], [ 0.0, 0.5],
        [ 0.0, 0.0], [0.5, 0.0], [ 0.5, 0.5],
        [ 0.5, 0.0], [1.0, 0.5], [ 0.5, 0.5],
        [ 0.5, 0.0], [1.0, 0.0], [ 1.0, 0.5],
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [2_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],
    ];   

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_o_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [0.0, 0.5], [0.5, 1.0], [0.0, 1.0],
        [0.0, 0.5], [0.5, 0.5], [0.5, 1.0],
        [0.0, 0.0], [0.5, 0.5], [0.0, 0.5],
        [0.0, 0.0], [0.5, 0.0], [0.5, 0.5],
        [0.5, 0.5], [1.0, 1.0], [0.5, 1.0],
        [0.5, 0.5], [1.0, 0.5], [1.0, 1.0],
        [0.5, 0.0], [1.0, 0.5], [0.5, 0.5],
        [0.5, 0.0], [1.0, 0.0], [1.0, 0.5],        
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [2_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],
    ];

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_s_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-0.5, 0.0], [0.0, 0.5], [-0.5, 0.5],
        [-0.5, 0.0], [0.0, 0.0], [ 0.0, 0.5],
        [ 0.0, 0.5], [0.5, 1.0], [ 0.0, 1.0],
        [ 0.0, 0.5], [0.5, 0.5], [ 0.5, 1.0],
        [ 0.0, 0.0], [0.5, 0.5], [ 0.0, 0.5],
        [ 0.0, 0.0], [0.5, 0.0], [ 0.5, 0.5],
        [ 0.5, 0.5], [1.0, 1.0], [ 0.5, 1.0],
        [ 0.5, 0.5], [1.0, 0.5], [ 1.0, 1.0],        
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [1_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
    ];
    
    ObjMesh::new(points, tex_coords)
}

fn create_geometry_l_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-0.5, 0.0], [0.0, 0.5], [-0.5, 0.5],
        [-0.5, 0.0], [0.0, 0.0], [ 0.0, 0.5],
        [ 0.0, 0.0], [0.5, 0.5], [ 0.0, 0.5],
        [ 0.0, 0.0], [0.5, 0.0], [ 0.5, 0.5],
        [ 0.5, 0.5], [1.0, 1.0], [ 0.5, 1.0],
        [ 0.5, 0.5], [1.0, 0.5], [ 1.0, 1.0],
        [ 0.5, 0.0], [1.0, 0.5], [ 0.5, 0.5],
        [ 0.5, 0.0], [1.0, 0.0], [ 1.0, 0.5],        
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
        [1_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
    ];

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_i_piece() -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [-1.0, 0.0], [-0.5, 0.5], [-1.0, 0.5],
        [-1.0, 0.0], [-0.5, 0.0], [-0.5, 0.5],
        [-0.5, 0.0], [ 0.0, 0.5], [-0.5, 0.5],
        [-0.5, 0.0], [ 0.0, 0.0], [ 0.0, 0.5],
        [ 0.0, 0.0], [ 0.5, 0.5], [ 0.0, 0.5],
        [ 0.0, 0.0], [ 0.5, 0.0], [ 0.5, 0.5],
        [ 0.5, 0.0], [ 1.0, 0.5], [ 0.5, 0.5],
        [ 0.5, 0.0], [ 1.0, 0.0], [ 1.0, 0.5],        
    ];
    let tex_coords: Vec<[f32; 2]> = vec![
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
        [0_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
    ];

    ObjMesh::new(points, tex_coords)
}

/// Create the model space geometry for the pieces displayed in the next panel
/// on the game's interface.
fn create_geometry_next_piece_panel() -> PieceMeshes {    
    PieceMeshes {
        t: create_geometry_t_piece(),
        j: create_geometry_j_piece(),
        z: create_geometry_z_piece(),
        o: create_geometry_o_piece(),
        s: create_geometry_s_piece(),
        l: create_geometry_l_piece(),
        i: create_geometry_i_piece(),
    }
}

fn create_textures_next_piece_panel() -> TexImage2D {
    let asset: &'static [u8; 1448] = include_asset!("blocks.png");
    teximage2d::load_from_memory(asset).unwrap().image
}

/// Send the shaders for a textbox buffer to the GPU.
fn send_to_gpu_shaders_next_piece_panel(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

#[derive(Copy, Clone)]
struct NextPiecePanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

fn create_buffers_geometry_piece_mesh() -> NextPiecePanelHandle {
    let v_pos_loc = 0;
    let v_tex_loc = 1;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    NextPiecePanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_piece_mesh(handle: NextPiecePanelHandle, mesh: &ObjMesh) {
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindVertexArray(handle.vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::VertexAttribPointer(handle.v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(handle.v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(handle.v_pos_loc);
        gl::EnableVertexAttribArray(handle.v_tex_loc);
    }
}

struct NextPanelHandles {
    t: NextPiecePanelHandle,
    j: NextPiecePanelHandle,
    z: NextPiecePanelHandle,
    o: NextPiecePanelHandle,
    s: NextPiecePanelHandle,
    l: NextPiecePanelHandle,
    i: NextPiecePanelHandle,
}

fn send_to_gpu_geometry_next_panel(meshes: &PieceMeshes) -> NextPanelHandles {
    let t_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(t_handle, &meshes.t);
    let j_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(j_handle, &meshes.j);
    let z_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(z_handle, &meshes.z);
    let o_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(o_handle, &meshes.o);
    let s_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(s_handle, &meshes.s);
    let l_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(l_handle, &meshes.l);
    let i_handle = create_buffers_geometry_piece_mesh();
    send_to_gpu_geometry_piece_mesh(i_handle, &meshes.i);

    NextPanelHandles {
        t: t_handle,
        j: j_handle,
        z: z_handle,
        o: o_handle,
        s: s_handle,
        l: l_handle,
        i: i_handle,
    }
}

struct PieceUniformsData {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_uniforms_next_piece_panel(
    piece: GooglyBlockPiece, scale: u32, viewport_width: u32, viewport_height: u32) -> PieceUniformsData {

    // FIXME: MAGIC NUMBERS IN USE HERE.
    let block_width = 2.0 * (scale as f32 / viewport_width as f32);
    let block_height = 2.0 * (scale as f32 / viewport_height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(block_width, block_height, 1.0);
    
    let trans_mat = match piece {
        GooglyBlockPiece::T => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::J => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::Z => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::O => Matrix4::from_translation(cgmath::vec3((0.50, 0.43, 0.0))),
        GooglyBlockPiece::S => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::L => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::I => Matrix4::from_translation(cgmath::vec3((0.555, 0.48, 0.0))),
    };

    PieceUniformsData {
        gui_scale_mat: gui_scale_mat,
        trans_mat: trans_mat,
    }
}

fn send_to_gpu_piece_uniforms(sp: GLuint, uniforms: &PieceUniformsData) {
    let gui_scale_mat_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_gui_scale").as_ptr())
    };
    debug_assert!(gui_scale_mat_loc > -1);
    let trans_mat_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_trans").as_ptr())
    };
    debug_assert!(trans_mat_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(gui_scale_mat_loc, 1, gl::FALSE, uniforms.gui_scale_mat.as_ptr());
        gl::UniformMatrix4fv(trans_mat_loc, 1, gl::FALSE, uniforms.trans_mat.as_ptr());
    }
}

fn send_to_gpu_uniforms_next_piece_panel(sp: GLuint, uniforms: &PieceUniformsData) {
    send_to_gpu_piece_uniforms(sp, uniforms);
}

fn update_uniforms_next_piece_panel(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let scale = 50;
    let gui_scale_x = 2.0 * (scale as f32) / (viewport_width as f32);
    let gui_scale_y = 2.0 * (scale as f32) / (viewport_height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
    let trans_mat = match game.context.borrow().next_block.borrow().block {
        GooglyBlockPiece::T => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::J => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::Z => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::O => Matrix4::from_translation(cgmath::vec3((0.50, 0.43, 0.0))),
        GooglyBlockPiece::S => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::L => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        GooglyBlockPiece::I => Matrix4::from_translation(cgmath::vec3((0.555, 0.48, 0.0))),
    };
    let uniforms = PieceUniformsData { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
    send_to_gpu_uniforms_next_piece_panel(game.ui.next_piece_panel.buffer.sp, &uniforms);
}

fn send_to_gpu_textures_next_piece_panel(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()  
}

struct GLNextPiecePanel {
    sp: GLuint,
    tex: GLuint,
    t_handle: NextPiecePanelHandle,
    j_handle: NextPiecePanelHandle,
    z_handle: NextPiecePanelHandle,
    o_handle: NextPiecePanelHandle,
    s_handle: NextPiecePanelHandle,
    l_handle: NextPiecePanelHandle,
    i_handle: NextPiecePanelHandle,
}

impl GLNextPiecePanel {
    fn handle(&self, piece: GooglyBlockPiece) -> NextPiecePanelHandle {
        match piece {
            GooglyBlockPiece::T => self.t_handle, 
            GooglyBlockPiece::J => self.j_handle,
            GooglyBlockPiece::Z => self.z_handle,
            GooglyBlockPiece::O => self.o_handle,
            GooglyBlockPiece::S => self.s_handle,
            GooglyBlockPiece::L => self.l_handle,
            GooglyBlockPiece::I => self.i_handle,
        }
    }
}

fn create_next_piece_panel_buffer(gl_context: &mut glh::GLState, uniforms: &PieceUniformsData) -> GLNextPiecePanel {
    let shader_source = create_shaders_next_piece_panel();
    let sp = send_to_gpu_shaders_next_piece_panel(gl_context, shader_source);
    let tex_image = create_textures_next_piece_panel();
    let tex = send_to_gpu_textures_next_piece_panel(&tex_image);
    let meshes = create_geometry_next_piece_panel();
    let handles = send_to_gpu_geometry_next_panel(&meshes);
    send_to_gpu_uniforms_next_piece_panel(sp, uniforms);

    GLNextPiecePanel {
        sp: sp,
        tex: tex,
        t_handle: handles.t,
        j_handle: handles.j,
        z_handle: handles.z,
        o_handle: handles.o,
        s_handle: handles.s,
        l_handle: handles.l,
        i_handle: handles.i,
    }
}

struct NextPiecePanel {
    current_piece: GooglyBlockPiece,
    buffer: GLNextPiecePanel,
}

impl NextPiecePanel {
    fn update(&mut self, piece: GooglyBlockPiece) {
        self.current_piece = piece;
    }
}

struct NextPiecePanelSpec {
    piece: GooglyBlockPiece,
}

fn load_next_piece_panel(
    game: &mut glh::GLState, 
    spec: NextPiecePanelSpec, uniforms: &PieceUniformsData) -> NextPiecePanel {
    
    let buffer = create_next_piece_panel_buffer(game, uniforms);
    NextPiecePanel {
        current_piece: spec.piece,
        buffer: buffer,
    }
}


fn create_shaders_playing_field() -> ShaderSource {
    let vert_source = include_shader!("playing_field.vert.glsl");
    let frag_source = include_shader!("playing_field.frag.glsl");

    ShaderSource {
        vert_name: "playing_field.vert.glsl",
        vert_source: vert_source,
        frag_name: "playing_field.frag.glsl",
        frag_source: frag_source,
    }
}

fn send_to_gpu_shaders_playing_field(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_geometry_playing_field(rows: usize, columns: usize) -> ObjMesh {
    let mut vertices: Vec<[GLfloat; 2]> = vec![];
    let width = 0.1;
    let height = 0.1;
    let top_left_x = -0.5;
    let top_left_y = 1.0;
    for row in 0..rows {
        for column in 0..columns {
            let row_f32 = row as f32;
            let col_f32 = column as f32;
            let top_left = [top_left_x + col_f32 * width, top_left_y - row_f32 * height];
            let bottom_left = [top_left_x + col_f32 * width, top_left_y - row_f32 * height - height];
            let top_right = [top_left_x + col_f32 * width + width, top_left_y - row_f32 * height];
            let bottom_right = [top_left_x + col_f32 * width + width, top_left_y - row_f32 * height - height];
            vertices.push(bottom_left);
            vertices.push(top_right);
            vertices.push(top_left);
            vertices.push(bottom_left);
            vertices.push(bottom_right);
            vertices.push(top_right);
        }
    }
    
    let mut tex_coords: Vec<[GLfloat; 2]> = vec![];
    for _row in 0..rows {
        for _column in 0..columns {
            tex_coords.push([1_f32 / 3_f32, 2_f32 / 3_f32]);
            tex_coords.push([2_f32 / 3_f32, 3_f32 / 3_f32]);
            tex_coords.push([1_f32 / 3_f32, 3_f32 / 3_f32]);
            tex_coords.push([1_f32 / 3_f32, 2_f32 / 3_f32]);
            tex_coords.push([2_f32 / 3_f32, 2_f32 / 3_f32]);
            tex_coords.push([2_f32 / 3_f32, 3_f32 / 3_f32]);
        }
    }

    ObjMesh::new(vertices, tex_coords)
}

#[derive(Copy, Clone)]
struct PlayingFieldBuffers {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

fn create_buffers_geometry_playing_field() -> PlayingFieldBuffers {
    let v_pos_loc = 0;
    let v_tex_loc = 1;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::CreateVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);

    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    PlayingFieldBuffers {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_playing_field(handle: PlayingFieldBuffers, mesh: &ObjMesh) {
    unsafe {
        gl::NamedBufferData(
            handle.v_pos_vbo, 
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::DYNAMIC_DRAW,
        );
        gl::NamedBufferData(
            handle.v_tex_vbo,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::DYNAMIC_DRAW
        );
        gl::BindVertexArray(handle.vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::VertexAttribPointer(handle.v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(handle.v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(handle.v_pos_loc);
        gl::EnableVertexAttribArray(handle.v_tex_loc);
    }
}

fn create_textures_playing_field() -> TexImage2D {
    let asset: &'static [u8; 1448] = include_asset!("blocks.png");
    teximage2d::load_from_memory(asset).unwrap().image
}

fn send_to_gpu_textures_playing_field(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

struct PlayingFieldUniforms {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_uniforms_playing_field(scale: u32, viewport_width: u32, viewport_height: u32) -> PlayingFieldUniforms {
    let gui_scale_x = (scale as f32) / (viewport_width as f32);
    let gui_scale_y = (scale as f32) / (viewport_height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
    let trans_mat = Matrix4::from_translation(cgmath::vec3((0.085, 0.0, 0.0)));
    
    PlayingFieldUniforms { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat }
}

fn update_uniforms_playing_field(game: &mut Game) {
    let viewport = game.viewport_dimensions();
    let scale = 488;
    let gui_scale_x = (scale as f32) / (viewport.width as f32);
    let gui_scale_y = (scale as f32) / (viewport.height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
    let trans_mat = Matrix4::from_translation(cgmath::vec3((0.085, 0.0, 0.0)));
    let uniforms = PlayingFieldUniforms { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
    send_to_gpu_uniforms_playing_field(game.ui.next_piece_panel.buffer.sp, uniforms);
}

fn send_to_gpu_uniforms_playing_field(sp: GLuint, uniforms: PlayingFieldUniforms) {
    let m_gui_scale_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_gui_scale").as_ptr())
    };
    debug_assert!(m_gui_scale_loc > -1);
    let m_trans_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_trans").as_ptr())
    };
    debug_assert!(m_trans_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(m_gui_scale_loc, 1, gl::FALSE, uniforms.gui_scale_mat.as_ptr());
        gl::UniformMatrix4fv(m_trans_loc, 1, gl::FALSE, uniforms.trans_mat.as_ptr());
    }    
}

struct PlayingFieldHandleSpec {
    rows: usize,
    columns: usize,
}

struct PlayingFieldHandle {
    sp: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    tex: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

impl PlayingFieldHandle {
    fn write(&mut self, tex_coords: &[[TextureQuad; 10]; 20]) -> io::Result<usize> {
        unsafe {
            gl::NamedBufferSubData(
                self.v_tex_vbo, 
                0,
                (mem::size_of::<TextureQuad>() * tex_coords[0].len() * tex_coords.len()) as GLsizeiptr,
                tex_coords.as_ptr() as *const GLvoid,
            );
        }
        let bytes_written = mem::size_of::<GLfloat>() * tex_coords.len();
        
        Ok(bytes_written)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct TextureQuad {
    inner: [[f32; 2]; 6],
}

impl TextureQuad {
    #[inline]
    fn new(top_left: [f32; 2], bottom_left: [f32; 2], bottom_right: [f32; 2], top_right: [f32; 2]) -> TextureQuad {
        TextureQuad {
            inner: [bottom_left, top_right, top_left, bottom_left, bottom_right, top_right]
        }
    }
}

struct PlayingField {
    tex_coords: [[TextureQuad; 10]; 20],
    gl_state: Rc<RefCell<glh::GLState>>,
    handle: PlayingFieldHandle,
}

impl PlayingField {
    fn new(gl_state: Rc<RefCell<glh::GLState>>, handle: PlayingFieldHandle) -> PlayingField {
        let quad = TextureQuad::new([0_f32, 0_f32], [0_f32, 0_f32], [0_f32, 0_f32], [0_f32, 0_f32]);
        PlayingField {
            tex_coords: [[quad; 10]; 20],
            gl_state: gl_state,
            handle: handle,
        }
    }

    fn write(&mut self, playing_field: &PlayingFieldState) -> io::Result<usize> {
        let rows = playing_field.landed_blocks.rows();
        let columns = playing_field.landed_blocks.columns();
        for row in 0..rows {
            for column in 0..columns {
                match playing_field.landed_blocks.get(row as isize, column as isize) {
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::EmptySpace) => {
                        let quad = TextureQuad::new(
                            [1_f32 / 3_f32, 3_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
                            [2_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 3_f32 / 3_f32]
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::T) => {
                        let quad = TextureQuad::new(
                            [0_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
                            [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::J) => {
                        let quad = TextureQuad::new(
                            [0_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
                            [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::Z) => {
                        let quad = TextureQuad::new(
                            [2_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
                            [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::O) => {
                        let quad = TextureQuad::new(
                            [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32],
                            [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::S) => {
                        let quad = TextureQuad::new(
                            [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32],
                            [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::L) => {
                        let quad = TextureQuad::new(
                            [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
                            [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    LandedBlocksQuery::InOfBounds(GooglyBlockElement::I) => {
                        let quad = TextureQuad::new(
                            [0_f32 / 3_f32, 0_f32 / 3_f32], [0_f32 / 3_f32, 0_f32 / 3_f32],
                            [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
                        );
                        self.tex_coords[row][column] = quad;
                    }
                    _ => {}
                }
            } 
        }

        let shape = playing_field.current_block.shape();
        let top_left_row = playing_field.current_position.row;
        let top_left_column = playing_field.current_position.column;
        let quad = match shape.element {
            GooglyBlockElement::EmptySpace => {
                TextureQuad::new(
                    [1_f32 / 3_f32, 3_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],
                    [2_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 3_f32 / 3_f32],               
                )
            }
            GooglyBlockElement::T => {
                TextureQuad::new(
                    [0_f32 / 3_f32, 3_f32 / 3_f32], [0_f32 / 3_f32, 2_f32 / 3_f32],
                    [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 3_f32 / 3_f32],
                )
            }
            GooglyBlockElement::J => {
                TextureQuad::new(
                    [0_f32 / 3_f32, 2_f32 / 3_f32], [0_f32 / 3_f32, 1_f32 / 3_f32],
                    [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 2_f32 / 3_f32],                    
                )
            }
            GooglyBlockElement::Z => {
                TextureQuad::new(
                    [2_f32 / 3_f32, 2_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],
                    [3_f32 / 3_f32, 1_f32 / 3_f32], [3_f32 / 3_f32, 2_f32 / 3_f32],                    
                )
            }
            GooglyBlockElement::O => {
                TextureQuad::new(
                    [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 0_f32 / 3_f32],
                    [3_f32 / 3_f32, 0_f32 / 3_f32], [3_f32 / 3_f32, 1_f32 / 3_f32],                    
                )
            }
            GooglyBlockElement::S => {
                TextureQuad::new(
                    [1_f32 / 3_f32, 1_f32 / 3_f32], [1_f32 / 3_f32, 0_f32 / 3_f32],
                    [2_f32 / 3_f32, 0_f32 / 3_f32], [2_f32 / 3_f32, 1_f32 / 3_f32],                    
                )
            }
            GooglyBlockElement::L => {
                TextureQuad::new(
                    [1_f32 / 3_f32, 2_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],
                    [2_f32 / 3_f32, 1_f32 / 3_f32], [2_f32 / 3_f32, 2_f32 / 3_f32],                    
                )
            }
            GooglyBlockElement::I => {
                TextureQuad::new(
                    [0_f32 / 3_f32, 0_f32 / 3_f32], [0_f32 / 3_f32, 0_f32 / 3_f32],
                    [1_f32 / 3_f32, 0_f32 / 3_f32], [1_f32 / 3_f32, 1_f32 / 3_f32],                    
                )
            }
        };
        for (shape_row, shape_column) in shape.iter() {
            let row = top_left_row + shape_row as isize;
            let column = top_left_column + shape_column as isize;
            self.tex_coords[row as usize][column as usize] = quad;
        }

        let bytes_written = mem::size_of::<TextureQuad>() * rows * columns;

        Ok(bytes_written)
    }

    fn send_to_gpu(&mut self) -> io::Result<usize> {
        self.handle.write(&self.tex_coords)?;
        let tex_coords_written = 6 * self.tex_coords.len();

        Ok(tex_coords_written)
    }
}

fn load_playing_field(game: &mut glh::GLState, spec: PlayingFieldHandleSpec, uniforms: PlayingFieldUniforms) -> PlayingFieldHandle {
    let shader_source = create_shaders_playing_field();
    let mesh = create_geometry_playing_field(spec.rows, spec.columns);
    let teximage = create_textures_playing_field();
    let sp = send_to_gpu_shaders_playing_field(game, shader_source);
    let handle = create_buffers_geometry_playing_field();
    send_to_gpu_geometry_playing_field(handle, &mesh);
    let tex = send_to_gpu_textures_playing_field(&teximage);
    send_to_gpu_uniforms_playing_field(sp, uniforms);

    PlayingFieldHandle {
        sp: sp,
        vao: handle.vao,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
        tex: tex,
        v_pos_loc: handle.v_pos_loc,
        v_tex_loc: handle.v_tex_loc,
    }
}



#[derive(Copy, Clone, Debug)]
struct GLTextBuffer {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

impl GLTextBuffer {
    fn write(&mut self, points: &[GLfloat], texcoords: &[GLfloat]) -> io::Result<usize> {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.v_pos_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER, (mem::size_of::<GLfloat>() * points.len()) as GLsizeiptr,
                points.as_ptr() as *const GLvoid, gl::DYNAMIC_DRAW
            );
            gl::BindBuffer(gl::ARRAY_BUFFER, self.v_tex_vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER, (mem::size_of::<GLfloat>() * texcoords.len()) as GLsizeiptr,
                texcoords.as_ptr() as *const GLvoid, gl::DYNAMIC_DRAW
            );
        }

        let bytes_written = mem::size_of::<GLfloat>() * (points.len() + texcoords.len());

        Ok(bytes_written)
    }
}

struct TextBuffer {
    points: Vec<f32>,
    tex_coords: Vec<f32>,
    gl_state: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    buffer: GLTextBuffer,
    scale_px: f32,
}

impl TextBuffer {
    fn new(
        gl_state: Rc<RefCell<glh::GLState>>, 
        atlas: Rc<BitmapFontAtlas>, 
        buffer: GLTextBuffer, scale_px: f32) -> TextBuffer {

        TextBuffer {
            points: vec![],
            tex_coords: vec![],
            gl_state: gl_state,
            atlas: atlas,
            buffer: buffer,
            scale_px: scale_px,
        }
    }

    fn clear(&mut self) {
        self.points.clear();
        self.tex_coords.clear();
    }

    fn write(&mut self, st: &[u8], placement: AbsolutePlacement) -> io::Result<(usize, usize)> {
        let atlas = &self.atlas;
        let scale_px = self.scale_px;
        let viewport_height: f32 = {
            let context = self.gl_state.borrow();
            context.height as f32
        };
        let viewport_width: f32 = {
            let context = self.gl_state.borrow();
            context.width as f32
        };

        let mut at_x = placement.x;
        let at_y = placement.y;

        for ch_i in st.iter() {
            let metadata_i = atlas.glyph_metadata[&(*ch_i as usize)];
            let atlas_col = metadata_i.column;
            let atlas_row = metadata_i.row;
            let atlas_rows = atlas.rows as f32;
            let atlas_columns = atlas.columns as f32;

            let s = (atlas_col as f32) * (1.0 / atlas_columns);
            let t = ((atlas_row + 1) as f32) * (1.0 / atlas_rows);

            let x_pos = at_x;
            let y_pos = at_y - (scale_px / viewport_height) * metadata_i.y_offset;

            at_x += metadata_i.width * (scale_px / viewport_width);

            self.points.push(x_pos);
            self.points.push(y_pos);
            self.points.push(x_pos);
            self.points.push(y_pos - scale_px / viewport_height);
            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos - scale_px / viewport_height);

            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos - scale_px / viewport_height);
            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos);
            self.points.push(x_pos);
            self.points.push(y_pos);
            
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t);
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t);            
            
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t);
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
        }

        let point_count = 6 * st.len();

        Ok((st.len(), point_count))
    }

    fn send_to_gpu(&mut self) -> io::Result<(usize, usize)> {
        self.buffer.write(&self.points, &self.tex_coords)?;
        let points_written = self.points.len();
        let tex_coords_written = self.tex_coords.len();
        
        Ok((points_written, tex_coords_written))
    }
}


#[derive(Copy, Clone, Debug)]
struct TextPanelUniforms {
    text_color: [f32; 4],
}

struct TextPanelSpec {
    atlas: Rc<BitmapFontAtlas>,
    score_placement: AbsolutePlacement,
    lines_placement: AbsolutePlacement,
    level_placement: AbsolutePlacement,
    tetrises_placement: AbsolutePlacement,
    t_placement: AbsolutePlacement,
    j_placement: AbsolutePlacement,
    z_placement: AbsolutePlacement,
    o_placement: AbsolutePlacement,
    s_placement: AbsolutePlacement,
    l_placement: AbsolutePlacement,
    i_placement: AbsolutePlacement,
    scale_px: f32,
}

#[derive(Copy, Clone, Debug)]
struct AbsolutePlacement {
    x: f32,
    y: f32,
}

struct TextElement7 {
    content: [u8; 7],
    placement: AbsolutePlacement,
}

impl TextElement7 {
    #[inline]
    fn write(&mut self, value: usize) {
        let d0 = value % 10;
        let d1 = ((value % 100) - d0) / 10;
        let d2 = ((value % 1000) - d1) / 100;
        let d3 = ((value % 10000) - d2) / 1000;
        let d4 = ((value % 100000) - d3) / 10000;
        let d5 = ((value % 1000000) - d4) / 100000;
        let d6 = ((value % 10000000) - d5) / 1000000;
        self.content[0] = d6 as u8 + 0x30;
        self.content[1] = d5 as u8 + 0x30;
        self.content[2] = d4 as u8 + 0x30;
        self.content[3] = d3 as u8 + 0x30;
        self.content[4] = d2 as u8 + 0x30;
        self.content[5] = d1 as u8 + 0x30;
        self.content[6] = d0 as u8 + 0x30;
    }
}

struct TextElement4 {
    content: [u8; 4],
    placement: AbsolutePlacement,
}

impl TextElement4 {
    #[inline]
    fn write(&mut self, value: usize) {
        let d0 = value % 10;
        let d1 = ((value % 100) - d0) / 10;
        let d2 = ((value % 1000) - d1) / 100;
        let d3 = ((value % 10000) - d2) / 1000;
        self.content[0] = d3 as u8 + 0x30;
        self.content[1] = d2 as u8 + 0x30;
        self.content[2] = d1 as u8 + 0x30;
        self.content[3] = d0 as u8 + 0x30;
    }
}

struct TextPanel {
    buffer: TextBuffer,
    score: TextElement7,
    level: TextElement4,
    tetrises: TextElement4,
    lines: TextElement4,
    t_pieces: TextElement4,
    j_pieces: TextElement4,
    z_pieces: TextElement4,
    o_pieces: TextElement4,
    s_pieces: TextElement4,
    l_pieces: TextElement4,
    i_pieces: TextElement4,
}

impl TextPanel {
    fn update_panel(&mut self) {
        self.buffer.clear();
        self.buffer.write(&self.score.content, self.score.placement).unwrap();
        self.buffer.write(&self.level.content, self.level.placement).unwrap();
        self.buffer.write(&self.tetrises.content, self.tetrises.placement).unwrap();
        self.buffer.write(&self.lines.content, self.lines.placement).unwrap();
        self.buffer.write(&self.t_pieces.content, self.t_pieces.placement).unwrap();
        self.buffer.write(&self.j_pieces.content, self.j_pieces.placement).unwrap();
        self.buffer.write(&self.z_pieces.content, self.z_pieces.placement).unwrap();
        self.buffer.write(&self.o_pieces.content, self.o_pieces.placement).unwrap();
        self.buffer.write(&self.s_pieces.content, self.s_pieces.placement).unwrap();
        self.buffer.write(&self.l_pieces.content, self.l_pieces.placement).unwrap();
        self.buffer.write(&self.i_pieces.content, self.i_pieces.placement).unwrap();
        self.buffer.send_to_gpu().unwrap();
    }

    fn update_score(&mut self, score: usize) {
        self.score.write(score);
    }

    fn update_level(&mut self, level: usize) {
        self.level.write(level);
    }

    fn update_lines(&mut self, lines: usize) {
        self.lines.write(lines);
    }

    fn update_tetrises(&mut self, tetrises: usize) {
        self.tetrises.write(tetrises);
    }

    fn update_t_pieces(&mut self, t_pieces: usize) {
        self.t_pieces.write(t_pieces);
    }

    fn update_j_pieces(&mut self, j_pieces: usize) {
        self.j_pieces.write(j_pieces);
    }
    
    fn update_z_pieces(&mut self, z_pieces: usize) {
        self.z_pieces.write(z_pieces);
    }

    fn update_o_pieces(&mut self, o_pieces: usize) {
        self.o_pieces.write(o_pieces);
    }

    fn update_s_pieces(&mut self, s_pieces: usize) {
        self.s_pieces.write(s_pieces);
    }

    fn update_l_pieces(&mut self, l_pieces: usize) {
        self.l_pieces.write(l_pieces);
    }

    fn update_i_pieces(&mut self, i_pieces: usize) {
        self.i_pieces.write(i_pieces);
    }

    fn update_statistics(&mut self, statistics: &Statistics) {
        self.update_t_pieces(statistics.t_pieces);
        self.update_j_pieces(statistics.j_pieces);
        self.update_z_pieces(statistics.z_pieces);
        self.update_o_pieces(statistics.o_pieces);
        self.update_s_pieces(statistics.s_pieces);
        self.update_l_pieces(statistics.l_pieces);
        self.update_i_pieces(statistics.i_pieces);
    }
}

struct TextBufferHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
}

/// Set up the geometry for rendering title screen text.
fn create_buffers_text_buffer() -> TextBufferHandle {
    let v_pos_loc = 0;
    let v_tex_loc = 1;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    TextBufferHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
    }
}

/// Load the shaders for a textbox buffer.
fn create_shaders_text_buffer() -> ShaderSource {
    let vert_source = include_shader!("text_panel.vert.glsl");
    let frag_source = include_shader!("text_panel.frag.glsl");

    ShaderSource { 
        vert_name: "text_panel.vert.glsl",
        vert_source: vert_source,
        frag_name: "text_panel.frag.glsl",
        frag_source: frag_source,
    }    
}

/// Send the shaders for a textbox buffer to the GPU.
fn send_to_gpu_shaders_text_buffer(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn send_to_gpu_uniforms_text_buffer(sp: GLuint, uniforms: TextPanelUniforms) {
    let text_color_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("text_color").as_ptr())
    };
    debug_assert!(text_color_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::Uniform4f(
            text_color_loc,
            uniforms.text_color[0], uniforms.text_color[1], 
            uniforms.text_color[2], uniforms.text_color[3]
        );
    }
}

fn create_text_buffer(
    gl_state: Rc<RefCell<glh::GLState>>, 
    atlas: Rc<BitmapFontAtlas>, scale_px: f32, uniforms: TextPanelUniforms) -> TextBuffer {
    
    let atlas_tex = send_to_gpu_font_texture(&atlas, gl::CLAMP_TO_EDGE).unwrap();
    let shader_source = create_shaders_text_buffer();
    let sp = {
        let mut context = gl_state.borrow_mut();
        send_to_gpu_shaders_text_buffer(&mut *context, shader_source)
    };
    let handle = create_buffers_text_buffer();
    send_to_gpu_uniforms_text_buffer(sp, uniforms);

    let buffer = GLTextBuffer {
        sp: sp,
        tex: atlas_tex,
        vao: handle.vao,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
    };

    TextBuffer::new(gl_state, atlas, buffer, scale_px)
}

fn load_text_panel(gl_state: Rc<RefCell<glh::GLState>>, spec: &TextPanelSpec, uniforms: TextPanelUniforms) -> TextPanel {
    let buffer = create_text_buffer(gl_state, spec.atlas.clone(), spec.scale_px, uniforms);
    let score = TextElement7 { content: [0; 7], placement: spec.score_placement };
    let lines =  TextElement4 { content: [0; 4], placement: spec.lines_placement };
    let level =  TextElement4 { content: [0; 4], placement: spec.level_placement };
    let tetrises = TextElement4 { content: [0; 4], placement: spec.tetrises_placement };
    let t_pieces = TextElement4 { content: [0; 4], placement: spec.t_placement };
    let j_pieces = TextElement4 { content: [0; 4], placement: spec.j_placement };
    let z_pieces = TextElement4 { content: [0; 4], placement: spec.z_placement };
    let o_pieces = TextElement4 { content: [0; 4], placement: spec.o_placement };
    let s_pieces = TextElement4 { content: [0; 4], placement: spec.s_placement };
    let l_pieces = TextElement4 { content: [0; 4], placement: spec.l_placement };
    let i_pieces = TextElement4 { content: [0; 4], placement: spec.i_placement };

    TextPanel {
        buffer: buffer,
        score: score,
        level: level,
        tetrises: tetrises,
        lines: lines,
        t_pieces: t_pieces,
        j_pieces: j_pieces,
        z_pieces: z_pieces,
        o_pieces: o_pieces,
        s_pieces: s_pieces,
        l_pieces: l_pieces,
        i_pieces: i_pieces,
    }
}

/// Load texture image into the GPU.
fn send_to_gpu_font_texture(atlas: &BitmapFontAtlas, wrapping_mode: GLuint) -> Result<GLuint, String> {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
    }
    debug_assert!(tex > 0);

    unsafe {
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA as i32, atlas.width as i32, atlas.height as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE,
            atlas.image.as_ptr() as *const GLvoid
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrapping_mode as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrapping_mode as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR as GLint);
    }

    let mut max_aniso = 0.0;
    unsafe {
        gl::GetFloatv(GL_MAX_TEXTURE_MAX_ANISOTROPY_EXT, &mut max_aniso);
        // Set the maximum!
        gl::TexParameterf(gl::TEXTURE_2D, GL_TEXTURE_MAX_ANISOTROPY_EXT, max_aniso);
    }

    Ok(tex)
}

/// Load a file atlas.
fn load_font_atlas() -> bmfa::BitmapFontAtlas {
    let asset: &'static [u8; 192197] = include_asset!("NotoSans-Bold.bmfa");
    let mut reader = io::Cursor::new(asset.iter());
    bmfa::from_reader(&mut reader).unwrap()
}

struct UI {
    ui_panel: UIPanel,
    text_panel: TextPanel,
    next_piece_panel: NextPiecePanel,
}

impl UI {
    fn update_panel(&mut self) {
        self.text_panel.update_panel();
    }

    fn update_score(&mut self, score: usize) {
        self.text_panel.update_score(score);
    }

    fn update_lines(&mut self, lines: usize) {
        self.text_panel.update_lines(lines);
    }

    fn update_level(&mut self, level: usize) {
        self.text_panel.update_level(level);
    }

    fn update_tetrises(&mut self, tetrises: usize) {
        self.text_panel.update_tetrises(tetrises);
    }

    fn update_statistics(&mut self, statistics: &Statistics) {
        self.text_panel.update_statistics(statistics);
    }

    fn update_next_piece(&mut self, piece: GooglyBlockPiece) {
        self.next_piece_panel.update(piece);
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Interval {
    Milliseconds(u64),
}

struct Timer {
    time: Duration,
    event_interval: Duration,
    event_count: u128,
}

impl Timer {
    fn new(interval: Interval) -> Timer {
        let event_interval = match interval {
            Interval::Milliseconds(millis) => Duration::from_millis(millis)
        };
        
        Timer {
            time: Duration::from_millis(0),
            event_interval: event_interval,
            event_count: 0,
        }
    }

    #[inline]
    fn update(&mut self, elapsed: Duration) {
        self.time += elapsed;
        self.event_count = self.time.as_millis() / self.event_interval.as_millis();
    }

    #[inline]
    fn event_triggered(&self) -> bool {
        self.event_count > 0
    }

    #[inline]
    fn reset(&mut self) {
        self.time = Duration::from_millis(0);
        self.event_count = 0;
    }
}

#[derive(Copy, Clone)]
struct PlayingFieldTimerSpec {
    fall_interval: Interval,
    collision_interval: Interval,
    left_hold_interval: Interval,
    right_hold_interval: Interval,
    down_hold_interval: Interval,
    rotate_interval: Interval,
}

struct PlayingFieldTimers {
    fall_timer: Timer,
    collision_timer: Timer,
    left_hold_timer: Timer,
    right_hold_timer: Timer,
    down_hold_timer: Timer,
    rotate_timer: Timer,
}

impl PlayingFieldTimers {
    fn new(spec: PlayingFieldTimerSpec) -> PlayingFieldTimers {
        PlayingFieldTimers {
            fall_timer: Timer::new(spec.fall_interval),
            collision_timer: Timer::new(spec.collision_interval),
            left_hold_timer: Timer::new(spec.left_hold_interval),
            right_hold_timer: Timer::new(spec.right_hold_interval),
            down_hold_timer: Timer::new(spec.down_hold_interval),
            rotate_timer: Timer::new(spec.rotate_interval),
        }
    }

    fn update(&mut self, elapsed: Duration) {
        self.fall_timer.update(elapsed);
        self.collision_timer.update(elapsed);
    }

    fn reset_input_timers(&mut self) {
        self.left_hold_timer.reset();
        self.right_hold_timer.reset();
        self.down_hold_timer.reset();
        self.rotate_timer.reset();
    }
}

struct Statistics {
    t_pieces: usize,
    j_pieces: usize,
    z_pieces: usize,
    o_pieces: usize,
    s_pieces: usize,
    l_pieces: usize,
    i_pieces: usize, 
}

impl Statistics {
    fn new() -> Statistics {
        Statistics {
            t_pieces: 0,
            j_pieces: 0,
            z_pieces: 0,
            o_pieces: 0,
            s_pieces: 0,
            l_pieces: 0,
            i_pieces: 0,
        }
    }

    fn update(&mut self, block: GooglyBlock) {
        match block.piece {
            GooglyBlockPiece::T => self.t_pieces += 1,
            GooglyBlockPiece::J => self.j_pieces += 1,
            GooglyBlockPiece::Z => self.z_pieces += 1,
            GooglyBlockPiece::O => self.o_pieces += 1,
            GooglyBlockPiece::S => self.s_pieces += 1,
            GooglyBlockPiece::L => self.l_pieces += 1,
            GooglyBlockPiece::I => self.i_pieces += 1,
        }
    }
}

struct ViewportDimensions {
    width: i32,
    height: i32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum InputAction {
    Unit,
    Press,
    Repeat,
    Release,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum InputKind {
    Unit,
    Left,
    Right,
    Down,
    Exit,
    Rotate,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Input {
    kind: InputKind,
    action: InputAction,
}

impl Input {
    #[inline]
    fn new(kind: InputKind, action: InputAction) -> Input {
        Input {
            kind: kind,
            action: action,
        }
    }
}

struct FallingState {
    context: Rc<RefCell<GameContext>>,
}

impl FallingState {
    fn new(context: Rc<RefCell<GameContext>>) -> FallingState {
        FallingState {
            context: context,
        }
    }

    fn handle_input(&mut self, input: Input, elapsed_milliseconds: Duration) {
        let context = self.context.borrow_mut();
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        match input.kind {
            InputKind::Left => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.left_hold_timer.update(elapsed_milliseconds);
                        if timers.left_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            let collides_with_left_element = playing_field_state.collides_with_element_to_the_left();
                            let collides_with_left_wall = playing_field_state.collides_with_left_wall();
                            if !collides_with_left_element || !collides_with_left_wall {
                                if collides_with_floor || collides_with_element {
                                    timers.fall_timer.reset();
                                }
                                playing_field_state.update_block_position(GooglyBlockMove::Left);
                            }
                            timers.left_hold_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            InputKind::Right => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.right_hold_timer.update(elapsed_milliseconds);
                        if timers.right_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            let collides_with_right_element = playing_field_state.collides_with_element_to_the_right();
                            let collides_with_right_wall = playing_field_state.collides_with_right_wall();
                            if !collides_with_right_element || !collides_with_right_wall {
                                if collides_with_floor || collides_with_element {
                                    timers.fall_timer.reset();
                                }
                                playing_field_state.update_block_position(GooglyBlockMove::Right);
                            }
                            timers.right_hold_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            InputKind::Down => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.down_hold_timer.update(elapsed_milliseconds);
                        if timers.down_hold_timer.event_triggered() {
                            let collides_with_floor = playing_field_state.collides_with_floor_below();
                            let collides_with_element = playing_field_state.collides_with_element_below();
                            if collides_with_floor || collides_with_element {
                                timers.fall_timer.reset();
                            }
                            playing_field_state.update_block_position(GooglyBlockMove::Down);
                            timers.down_hold_timer.reset();
                        }                        
                    }
                    _ => {}
                }
            }
            InputKind::Rotate => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        timers.rotate_timer.update(elapsed_milliseconds);
                        if timers.rotate_timer.event_triggered() {
                            playing_field_state.update_block_position(GooglyBlockMove::Rotate);
                            timers.rotate_timer.reset();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        } 
    }

    fn update(&mut self, elapsed_milliseconds: Duration) {
        let context = self.context.borrow();
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        let mut statistics = context.statistics.borrow_mut();
        let mut next_block = context.next_block.borrow_mut();

        let collides_with_floor = playing_field_state.collides_with_floor_below();
        let collides_with_element = playing_field_state.collides_with_element_below();

        timers.fall_timer.update(elapsed_milliseconds);
        // Update the game world.
        if collides_with_floor || collides_with_element {
            timers.collision_timer.update(elapsed_milliseconds);
        } else {
            timers.collision_timer.reset();
        }

        if timers.fall_timer.event_triggered() {
            playing_field_state.update_block_position(GooglyBlockMove::Fall);
            timers.fall_timer.reset();
        }

        if timers.collision_timer.event_triggered() {
            let current_block = playing_field_state.current_block;
            playing_field_state.update_landed();
            statistics.update(current_block);
            let old_block = next_block.block;
            next_block.update();
            let new_block = GooglyBlock::new(old_block, GooglyBlockRotation::R0);
            playing_field_state.update_new_block(new_block);
            timers.collision_timer.reset();
        }


    }
}

struct ClearingState {
    context: Rc<RefCell<GameContext>>,
}

impl ClearingState {
    fn new(context: Rc<RefCell<GameContext>>) -> ClearingState {
        ClearingState {
            context: context,
        }
    }

    fn handle_input(&mut self, input: Input, elapsed_milliseconds: Duration) {

    }

    fn update(&mut self, elapsed_milliseconds: Duration) {

    }
}


enum GameState {
    Falling(FallingState),
    Clearing(ClearingState),
}

impl GameState {
    fn handle_input(&mut self, input: Input, elapsed_milliseconds: Duration) {
        match *self {
            GameState::Falling(ref mut s) => s.handle_input(input, elapsed_milliseconds),
            GameState::Clearing(ref mut s) => s.handle_input(input, elapsed_milliseconds),
        }
    }

    fn update(&mut self, elapsed_milliseconds: Duration) {
        match *self {
            GameState::Falling(ref mut s) => s.update(elapsed_milliseconds),
            GameState::Clearing(ref mut s) => s.update(elapsed_milliseconds),
        }
    }
}

struct NextBlockCell {
    block: GooglyBlockPiece,
}

impl NextBlockCell {
    fn new(block: GooglyBlockPiece) -> NextBlockCell {
        NextBlockCell {
            block: block,
        }
    }

    fn update(&mut self) {
        let mut rng = rng::thread_rng();
        let random = rng.gen_range::<u32, u32, u32>(0, 7);
        self.block = match random {
            0 => GooglyBlockPiece::T,
            1 => GooglyBlockPiece::J,
            2 => GooglyBlockPiece::Z,
            3 => GooglyBlockPiece::O,
            4 => GooglyBlockPiece::S,
            5 => GooglyBlockPiece::L,
            6 => GooglyBlockPiece::I,
            _ => GooglyBlockPiece::T,
        };
    }
}

struct GameContext {
    gl: Rc<RefCell<glh::GLState>>,
    timers: Rc<RefCell<PlayingFieldTimers>>,
    playing_field_state: Rc<RefCell<PlayingFieldState>>,
    next_block: Rc<RefCell<NextBlockCell>>,
    statistics: Rc<RefCell<Statistics>>,
}

struct Game {
    context: Rc<RefCell<GameContext>>,
    state: GameState,
    playing_field: PlayingField,
    ui: UI,
    background: BackgroundPanel,
    score: usize,
    level: usize,
    lines: usize,
    tetrises: usize,
}

impl Game {
    #[inline]
    fn get_framebuffer_size(&self) -> (i32, i32) {
        self.context.borrow().gl.borrow_mut().window.get_framebuffer_size()
    }

    #[inline]
    fn window_should_close(&self) -> bool {
        self.context.borrow().gl.borrow_mut().window.should_close()
    }

    #[inline]
    fn window_set_should_close(&mut self, close: bool) {
        self.context.borrow_mut().gl.borrow_mut().window.set_should_close(close);
    }

    #[inline]
    fn update_fps_counter(&mut self) {
        let game_context = self.context.borrow_mut();
        let mut gpu_context = game_context.gl.borrow_mut();
        glh::update_fps_counter(&mut gpu_context);
    }

    #[inline]
    fn update_timers(&mut self) -> Duration {
        let game_context = self.context.borrow_mut();
        let mut gpu_context = game_context.gl.borrow_mut();
        let elapsed_seconds = glh::update_timers(&mut gpu_context);

        Duration::from_millis((elapsed_seconds * 1000_f64) as u64)
    }

    #[inline]
    fn swap_buffers(&mut self) {
        self.context.borrow_mut().gl.borrow_mut().window.swap_buffers();
    }

    #[inline]
    fn update_background(&mut self) {
        update_uniforms_background_panel(self);
    }

    fn render_background(&mut self) {
        unsafe {
            gl::UseProgram(self.background.buffer.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.background.buffer.tex);
            gl::BindVertexArray(self.background.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn update_ui(&mut self) {
        update_ui_panel_uniforms(self);
        update_uniforms_next_piece_panel(self);
        self.ui.update_score(self.score);
        self.ui.update_lines(self.lines);
        self.ui.update_level(self.level);
        self.ui.update_tetrises(self.tetrises);
        self.ui.update_statistics(&self.context.borrow().statistics.borrow());
        self.ui.update_next_piece(self.context.borrow().next_block.borrow().block);
        self.ui.update_panel();
    }

    fn render_ui(&mut self) {
        unsafe {
            // Render the game board. We turn off depth testing to do so since this is
            // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
            // from rendering the game board.
            gl::UseProgram(self.ui.ui_panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.ui_panel.tex);
            gl::BindVertexArray(self.ui.ui_panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(self.ui.text_panel.buffer.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.text_panel.buffer.buffer.tex);
            gl::BindVertexArray(self.ui.text_panel.buffer.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 47 * 6);

            gl::UseProgram(self.ui.next_piece_panel.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.next_piece_panel.buffer.tex);
            gl::BindVertexArray(self.ui.next_piece_panel.buffer.handle(self.context.borrow().next_block.borrow().block).vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * 8);
        }
    }

    #[inline]
    fn poll_events(&mut self) {
        self.context.borrow_mut().gl.borrow_mut().glfw.poll_events();
    }

    #[inline]
    fn get_key(&self, key: Key) -> Action {
        self.context.borrow().gl.borrow().window.get_key(key)
    }

    #[inline]
    fn viewport_dimensions(&self) -> ViewportDimensions {
        let (width, height) = {
            let game_context = self.context.borrow();
            let context = game_context.gl.borrow();
            (context.width as i32, context.height as i32)
        };
        
        ViewportDimensions { 
            width: width, 
            height: height,
        }
    }

    #[inline]
    fn update_framebuffer_size(&mut self) {
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let dims = self.viewport_dimensions();
        if (dims.width != viewport_width) && (dims.height != viewport_height) {
            glfw_framebuffer_size_callback(
                self, viewport_width as u32, viewport_height as u32
            );
        }
    }

    fn update_timers_playing_field(&mut self, elapsed: Duration) {
        self.context.borrow_mut().timers.borrow_mut().update(elapsed);
    }

    fn update_playing_field(&mut self) {
        update_uniforms_playing_field(self);
        let context = self.context.borrow();
        let playing_field_state = context.playing_field_state.borrow();
        self.playing_field.write(&playing_field_state).unwrap();
        self.playing_field.send_to_gpu().unwrap();
    }

    fn update_next_piece(&mut self) {
        self.context.borrow_mut().next_block.borrow_mut().update();
    }

    fn render_playing_field(&mut self) {
        unsafe {
            gl::UseProgram(self.playing_field.handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.playing_field.handle.tex);
            gl::BindVertexArray(self.playing_field.handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 2 * 6 * 20 * 10);
            gl::Disable(gl::BLEND);
        }
    }

    #[inline]
    fn clear_depth_buffer(&mut self) {
        unsafe {
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
        }
    }

    #[inline]
    fn clear_frame_buffer(&mut self) {
        unsafe {
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
        }
    }

    #[inline]
    fn update_viewport(&mut self) {
        let dims = self.viewport_dimensions();
        unsafe {
            gl::Viewport(0, 0, dims.width, dims.height);
        }
    }

    #[inline]
    fn init_gpu(&mut self) {
        unsafe {
            // Enable depth testing.
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LESS);
            // Clear the z-buffer and the frame buffer.
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
    
            let dims = self.viewport_dimensions();
            gl::Viewport(0, 0, dims.width, dims.height);
        }
    }
}

/// The GLFW frame buffer size callback function. This is normally set using
/// the GLFW `glfwSetFramebufferSizeCallback` function, but instead we explicitly
/// handle window resizing in our state updates on the application side. Run this function
/// whenever the size of the viewport changes.
#[inline]
fn glfw_framebuffer_size_callback(game: &mut Game, width: u32, height: u32) {
    let game_context = game.context.borrow_mut();
    let mut context = game_context.gl.borrow_mut();
    context.width = width;
    context.height = height;
}

/// Initialize the logger.
fn init_logger(log_file: &str) {
    file_logger::init(log_file).expect("Failed to initialize logger.");
}

/// Create and OpenGL context.
fn init_gl(width: u32, height: u32) -> glh::GLState {
    let gl_state = match glh::start_gl(width, height) {
        Ok(val) => val,
        Err(e) => {
            panic!("Failed to Initialize OpenGL context. Got error: {}", e);
        }
    };

    gl_state
}

fn init_game() -> Game {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let width = 896;
    let height = 504;
    let gl_context = Rc::new(RefCell::new(init_gl(width, height)));
    let atlas = Rc::new(load_font_atlas());

    let background_panel_height = height as usize;
    let background_panel_width = width as usize;
    let background_panel_spec = BackgroundPanelSpec { height: background_panel_height, width: background_panel_width };
    let background = {
        let mut context = gl_context.borrow_mut(); 
        load_background_panel(&mut *context, background_panel_spec)
    };
    let (viewport_width, viewport_height) = {
        let context = gl_context.borrow();
        context.window.get_framebuffer_size()
    };
    let viewport_width = viewport_width as f32;
    let viewport_height = viewport_height as f32;
    let panel_width = 642;
    let panel_height = 504;
    let gui_scale_x = (panel_width as f32) / viewport_width;
    let gui_scale_y = (panel_height as f32) / viewport_height;

    let ui_panel_spec = UIPanelSpec { height: panel_height, width: panel_width };
    let ui_panel_uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    let ui_panel = {
        let mut context = gl_context.borrow_mut();
        load_ui_panel(&mut *context, ui_panel_spec, ui_panel_uniforms)
    };
    
    let text_panel_uniforms = TextPanelUniforms { text_color: TEXT_COLOR };
    let text_panel_spec = TextPanelSpec {
        atlas: atlas.clone(),
        score_placement: AbsolutePlacement { x: 0.46, y: 0.11 },
        level_placement: AbsolutePlacement { x: 0.50, y: -0.21 },
        lines_placement: AbsolutePlacement { x: 0.50, y: -0.54 },
        tetrises_placement: AbsolutePlacement { x: 0.50, y: -0.87 },
        t_placement: AbsolutePlacement { x: -0.41, y:  0.62 },
        j_placement: AbsolutePlacement { x: -0.41, y:  0.38 },
        z_placement: AbsolutePlacement { x: -0.41, y:  0.15 },
        o_placement: AbsolutePlacement { x: -0.41, y: -0.08 },
        s_placement: AbsolutePlacement { x: -0.41, y: -0.29 },
        l_placement: AbsolutePlacement { x: -0.41, y: -0.52 },
        i_placement: AbsolutePlacement { x: -0.41, y: -0.74 },
        scale_px: 48.0,
    };
    let text_panel = load_text_panel(gl_context.clone(), &text_panel_spec, text_panel_uniforms);
    
    let next_piece = GooglyBlockPiece::T;
    let next_piece_panel_spec = NextPiecePanelSpec {
        piece: next_piece,
    };
    let next_piece_panel_uniforms = create_uniforms_next_piece_panel(next_piece, 50, width, height);
    let next_piece_panel = {
        let mut context = gl_context.borrow_mut();
        load_next_piece_panel(&mut *context, next_piece_panel_spec, &next_piece_panel_uniforms)
    };
    let ui = UI { 
        ui_panel: ui_panel,
        text_panel: text_panel,
        next_piece_panel: next_piece_panel,
    };
    
    let playing_field_uniforms = create_uniforms_playing_field(488, viewport_width as u32, viewport_height as u32);
    let playing_field_spec = PlayingFieldHandleSpec {
        rows: 20,
        columns: 10,
    };
    let playing_field_handle = {
        let mut context = gl_context.borrow_mut();    
        load_playing_field(&mut *context, playing_field_spec, playing_field_uniforms)
    };
    let starting_block = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
    let starting_position = BlockPosition::new(0, 4);
    let playing_field_state = Rc::new(RefCell::new(PlayingFieldState::new(starting_block, starting_position)));
    let playing_field = PlayingField::new(gl_context.clone(), playing_field_handle);
    let timer_spec = PlayingFieldTimerSpec {
        fall_interval: Interval::Milliseconds(500),
        collision_interval: Interval::Milliseconds(500),
        left_hold_interval: Interval::Milliseconds(70),
        right_hold_interval: Interval::Milliseconds(70),
        down_hold_interval: Interval::Milliseconds(50),
        rotate_interval: Interval::Milliseconds(100),
    };
    let next_block_cell = Rc::new(RefCell::new(NextBlockCell::new(next_piece)));
    let timers = Rc::new(RefCell::new(PlayingFieldTimers::new(timer_spec)));
    let statistics = Rc::new(RefCell::new(Statistics::new()));
    let context = Rc::new(RefCell::new(GameContext {
        gl: gl_context,
        timers: timers,
        playing_field_state: playing_field_state,
        statistics: statistics,
        next_block: next_block_cell,
    }));
    let state = GameState::Falling(FallingState::new(context.clone()));

    Game {
        context: context,
        state: state,
        playing_field: playing_field,
        ui: ui,
        background: background,
        score: 0,
        level: 0,
        lines: 0,
        tetrises: 0,
    }
}

fn main() {
    let mut game = init_game();
    game.init_gpu();
    while !game.window_should_close() {
        // Check input.
        let elapsed_milliseconds = game.update_timers();

        game.poll_events();
        match game.get_key(Key::Escape) {
            Action::Press | Action::Repeat => {
                game.window_set_should_close(true);
            }
            _ => {}
        }
        match game.get_key(Key::Left) {
            Action::Press => {
                let input = Input::new(InputKind::Left, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Left, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Right) {
            Action::Press => {
                let input = Input::new(InputKind::Right, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Right, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Down) {
            Action::Press => {
                let input = Input::new(InputKind::Down, InputAction::Press);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Down, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::R) {
            Action::Press => {
                let input = Input::new(InputKind::Rotate, InputAction::Press);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Rotate, InputAction::Repeat);
                game.state.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }

        game.state.update(elapsed_milliseconds);
        game.update_fps_counter();
        game.update_framebuffer_size();

        // Render the results.
        game.clear_frame_buffer();
        game.clear_depth_buffer();
        game.update_viewport();
        game.update_background();
        game.render_background();
        game.update_ui();
        game.render_ui();
        game.update_playing_field();
        game.render_playing_field();

        // Send the results to the output.
        game.swap_buffers();
    }

    info!("END LOG");
}
