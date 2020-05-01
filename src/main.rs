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
extern crate tex_atlas;


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
use tex_atlas::TextureAtlas2D;
use playing_field::{
    BlockPosition, GooglyBlock, PlayingFieldState,
    GooglyBlockPiece, GooglyBlockRotation, GooglyBlockElement, GooglyBlockMove,
    PlayingFieldStateSpec,
};
use rand::prelude as rng;
use rand::distributions::{Distribution, Uniform};

use std::io;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::Duration;
use std::collections::hash_map::HashMap;


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
fn send_to_gpu_texture(atlas: &TextureAtlas2D, wrapping_mode: GLuint) -> Result<GLuint, String> {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA as i32, atlas.width as i32, atlas.height as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE,
            atlas.as_ptr() as *const GLvoid
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

fn create_background_panel_atlas() -> TextureAtlas2D {
    let asset = include_asset!("background.atlas");
    let atlas = tex_atlas::load_from_memory(asset).unwrap().atlas;

    atlas
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

fn create_title_texture_atlas() -> TextureAtlas2D {
    let asset = include_asset!("title.atlas");
    tex_atlas::load_from_memory(asset).unwrap().atlas
}

fn send_to_gpu_textures_background(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct BackgroundPanelUniforms { 
    gui_scale_mat: Matrix4,
}

fn send_to_gpu_uniforms_background_panel(sp: GLuint, uniforms: BackgroundPanelUniforms) {
    let m_gui_scale_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_gui_scale").as_ptr())
    };
    debug_assert!(m_gui_scale_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(m_gui_scale_loc, 1, gl::FALSE, uniforms.gui_scale_mat.as_ptr());
    }
}

#[derive(Copy, Clone)]
struct BackgroundPanelSpec<'a> { 
    height: usize, 
    width: usize,
    background_atlas: &'a TextureAtlas2D,
    title_atlas: &'a TextureAtlas2D,
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
    background_handle: GLBackgroundPanel,
    title_handle: GLBackgroundPanel,
}

fn load_background_panel(game: &mut glh::GLState, spec: BackgroundPanelSpec) -> BackgroundPanel {
    let shader_source = create_shaders_background();
    let mesh = create_geometry_background();
    let sp = send_to_gpu_shaders_background(game, shader_source);
    let background_buffer = create_buffers_geometry_background();
    send_to_gpu_geometry_background(background_buffer, &mesh);
    let background_tex = send_to_gpu_textures_background(&spec.background_atlas);
    let background_handle = GLBackgroundPanel {
        sp: sp,
        v_pos_vbo: background_buffer.v_pos_vbo,
        v_tex_vbo: background_buffer.v_tex_vbo,
        vao: background_buffer.vao,
        tex: background_tex,
    };
    let title_buffer = create_buffers_geometry_background();
    send_to_gpu_geometry_background(title_buffer, &mesh);
    let title_tex = send_to_gpu_textures_background(&spec.title_atlas);
    let title_handle = GLBackgroundPanel {
        sp: sp,
        v_pos_vbo: title_buffer.v_pos_vbo,
        v_tex_vbo: title_buffer.v_tex_vbo,
        vao: title_buffer.vao,
        tex: title_tex,
    };

    BackgroundPanel {
        height: spec.height,
        width: spec.width,
        background_handle: background_handle,
        title_handle: title_handle,
    }
}
























#[derive(Copy, Clone)]
struct TitleScreenBackgroundBuffers {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

#[derive(Copy, Clone)]
struct TitleScreenBufferHandle {
    sp: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
    tex: GLuint,  
}

#[derive(Copy, Clone)]
struct TitleScreenBackgroundHandle {
    width: usize,
    height: usize,
    handle: TitleScreenBufferHandle,
}

#[derive(Copy, Clone)]
struct TitleScreenBackgroundUniforms {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_shaders_title_screen_background() -> ShaderSource {
    let vert_source = include_shader!("title_screen_background.vert.glsl");
    let frag_source = include_shader!("title_screen_background.frag.glsl");

    ShaderSource { 
        vert_name: "title_screen_background.vert.glsl",
        vert_source: vert_source, 
        frag_name: "title_screen_background.frag.glsl",
        frag_source: frag_source 
    }
}

fn create_geometry_title_screen_background(atlas: &TextureAtlas2D) -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [1_f32, 1_f32], [-1_f32, -1_f32], [ 1_f32, -1_f32],
        [1_f32, 1_f32], [-1_f32,  1_f32], [-1_f32, -1_f32],
    ];
    let corners = atlas.get_name_corners_uv("title").unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        top_right, bottom_left, bottom_right,
        top_right, top_left, bottom_left
    ];

    ObjMesh::new(points, tex_coords)
}

fn send_to_gpu_shaders_title_screen_background(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_buffers_geometry_title_screen_background() -> TitleScreenBackgroundBuffers {
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

    TitleScreenBackgroundBuffers {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_title_screen_background(handle: TitleScreenBackgroundBuffers, mesh: &ObjMesh) {
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

fn send_to_gpu_textures_title_screen_background(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}


fn send_to_gpu_uniforms_title_screen_background(sp: GLuint, uniforms: TitleScreenBackgroundUniforms) {
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

#[derive(Copy, Clone)]
struct TitleScreenFlashingBuffers {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

#[derive(Copy, Clone)]
struct TitleScreenFlashingBufferHandle {
    sp: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
    tex: GLuint,  
}

#[derive(Copy, Clone)]
struct TitleScreenFlashingHandle {
    width: usize,
    height: usize,
    placement: AbsolutePlacement,
    handle: TitleScreenFlashingBufferHandle,
}

struct TitleScreenFlashingUniforms {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_shaders_title_screen_flashing() -> ShaderSource {
    let vert_source = include_shader!("title_screen_background.vert.glsl");
    let frag_source = include_shader!("title_screen_background.frag.glsl");

    ShaderSource { 
        vert_name: "title_screen_background.vert.glsl",
        vert_source: vert_source, 
        frag_name: "title_screen_background.frag.glsl",
        frag_source: frag_source 
    }
}

fn create_geometry_title_screen_flashing(atlas: &TextureAtlas2D) -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [1_f32, 1_f32], [-1_f32, -1_f32], [ 1_f32, -1_f32],
        [1_f32, 1_f32], [-1_f32,  1_f32], [-1_f32, -1_f32],
    ];
    let corners = atlas.get_name_corners_uv("PressEnter").unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        top_right, bottom_left, bottom_right,
        top_right, top_left, bottom_left
    ];

    ObjMesh::new(points, tex_coords)
}

fn send_to_gpu_shaders_title_screen_flashing(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_buffers_geometry_title_screen_flashing() -> TitleScreenFlashingBuffers {
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

    TitleScreenFlashingBuffers {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_title_screen_flashing(handle: TitleScreenFlashingBuffers, mesh: &ObjMesh) {
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

fn send_to_gpu_textures_title_screen_flashing(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}


struct TitleScreenSpec<'a, 'b> {
    background_width: usize,
    background_height: usize,
    background_atlas: &'a TextureAtlas2D,
    flashing_width: usize,
    flashing_height: usize,
    flashing_placement: AbsolutePlacement,
    flashing_atlas: &'b TextureAtlas2D,
}

struct TitleScreenHandle {
    background_handle: TitleScreenBackgroundHandle,
    flashing_handle: TitleScreenFlashingHandle,
}

fn send_to_gpu_uniforms_title_screen_flashing(sp: GLuint, uniforms: TitleScreenFlashingUniforms) {
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

fn load_title_screen(game: &mut glh::GLState, spec: TitleScreenSpec) -> TitleScreenHandle {
    let background_source = create_shaders_title_screen_background();
    let background_mesh = create_geometry_title_screen_background(&spec.background_atlas);
    let flashing_source = create_shaders_title_screen_flashing();
    let flashing_mesh = create_geometry_title_screen_flashing(&spec.flashing_atlas);
    let background_buffers = create_buffers_geometry_title_screen_background();
    let flashing_buffers = create_buffers_geometry_title_screen_flashing();
    let background_sp = send_to_gpu_shaders_title_screen_background(game, background_source);
    let flashing_sp = send_to_gpu_shaders_title_screen_flashing(game, flashing_source);
    send_to_gpu_geometry_title_screen_background(background_buffers, &background_mesh);
    send_to_gpu_geometry_title_screen_flashing(flashing_buffers, &flashing_mesh);
    let background_tex = send_to_gpu_textures_title_screen_background(&spec.background_atlas);
    let flashing_tex = send_to_gpu_textures_title_screen_flashing(&spec.flashing_atlas);
    let background_buffers_handle = TitleScreenBufferHandle {
        sp: background_sp,
        vao: background_buffers.vao,
        v_pos_vbo: background_buffers.v_pos_vbo,
        v_tex_vbo: background_buffers.v_tex_vbo,
        v_pos_loc: background_buffers.v_pos_loc,
        v_tex_loc: background_buffers.v_tex_loc,
        tex: background_tex,  
    };
    let flashing_buffers_handle = TitleScreenFlashingBufferHandle {
        sp: flashing_sp,
        vao: flashing_buffers.vao,
        v_pos_vbo: flashing_buffers.v_pos_vbo,
        v_tex_vbo: flashing_buffers.v_tex_vbo,
        v_pos_loc: flashing_buffers.v_pos_loc,
        v_tex_loc: flashing_buffers.v_tex_loc,
        tex: flashing_tex,  
    };
    let background_handle = TitleScreenBackgroundHandle {
        width: spec.background_width,
        height: spec.background_height,
        handle: background_buffers_handle,
    };
    let flashing_handle = TitleScreenFlashingHandle {
        width: spec.flashing_width,
        height: spec.flashing_height,
        placement: spec.flashing_placement,
        handle: flashing_buffers_handle,
    };

    TitleScreenHandle {
        background_handle: background_handle,
        flashing_handle: flashing_handle,
    }
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

fn create_geometry_ui_panel(atlas: &TextureAtlas2D) -> ObjMesh {
    let points: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [-1.0, -1.0], [ 1.0, -1.0],
        [1.0, 1.0], [-1.0,  1.0], [-1.0, -1.0]
    ];
    let corners: tex_atlas::BoundingBoxCornersTexCoords = atlas.get_name_corners_uv("Panel").unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        top_right, bottom_left, bottom_right, top_right, top_left, bottom_left,
        top_right, bottom_left, bottom_right, top_right, top_left, bottom_left,
        top_right, bottom_left, bottom_right, top_right, top_left, bottom_left,
        top_right, bottom_left, bottom_right, top_right, top_left, bottom_left,
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

fn create_atlas_ui_panel() -> TextureAtlas2D {
    let asset = include_asset!("ui_panel.atlas");
    let atlas = tex_atlas::load_from_memory(asset).unwrap().atlas;

    atlas
}

fn send_to_gpu_textures_ui_panel(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct UIPanelSpec<'a> {
    height: usize,
    width: usize,
    atlas: &'a TextureAtlas2D,
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
    trans_mat: Matrix4,
    gui_scale_mat: Matrix4,
}

fn send_to_gpu_uniforms_ui_panel(sp: GLuint, uniforms: UIPanelUniforms) {
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
        ptr::copy(&uniforms.trans_mat, mem::transmute(&mut buffer[offsets[1] as usize]), 1);
        ptr::copy(&uniforms.gui_scale_mat, mem::transmute(&mut buffer[offsets[0] as usize]), 1);
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
    let mesh = create_geometry_ui_panel(&spec.atlas);
    let handle = create_buffers_geometry_ui_panel();
    send_to_gpu_geometry_ui_panel(handle, &mesh);
    let tex = send_to_gpu_textures_ui_panel(&spec.atlas);
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

fn generate_texture_coords_block(atlas: &TextureAtlas2D, name: &str) -> Vec<[f32; 2]> {
    let corners: tex_atlas::BoundingBoxCornersTexCoords = atlas.get_name_corners_uv(name).unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        bottom_left, top_right, top_left, bottom_left, bottom_right, top_right,
        bottom_left, top_right, top_left, bottom_left, bottom_right, top_right,
        bottom_left, top_right, top_left, bottom_left, bottom_right, top_right,
        bottom_left, top_right, top_left, bottom_left, bottom_right, top_right,
    ];

    tex_coords
}

fn create_geometry_t_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "t_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_j_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "j_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_z_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "z_piece"); 

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_o_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "o_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_s_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "s_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_l_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "l_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_geometry_i_piece(atlas: &TextureAtlas2D) -> ObjMesh {
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
    let tex_coords = generate_texture_coords_block(atlas, "i_piece");

    ObjMesh::new(points, tex_coords)
}

fn create_block_texture_atlas() -> TextureAtlas2D {
    let source = include_asset!("block_textures.atlas");
    let atlas = tex_atlas::load_from_memory(source).unwrap().atlas;

    atlas
}

/// Create the model space geometry for the pieces displayed in the next panel
/// on the game's interface.
fn create_geometry_next_piece_panel(atlas: &TextureAtlas2D) -> PieceMeshes {    
    PieceMeshes {
        t: create_geometry_t_piece(atlas),
        j: create_geometry_j_piece(atlas),
        z: create_geometry_z_piece(atlas),
        o: create_geometry_o_piece(atlas),
        s: create_geometry_s_piece(atlas),
        l: create_geometry_l_piece(atlas),
        i: create_geometry_i_piece(atlas),
    }
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


fn send_to_gpu_textures_next_piece_panel(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()  
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

fn create_next_piece_panel_buffer(gl_context: &mut glh::GLState, atlas: &TextureAtlas2D, uniforms: &PieceUniformsData) -> GLNextPiecePanel {
    let shader_source = create_shaders_next_piece_panel();
    let sp = send_to_gpu_shaders_next_piece_panel(gl_context, shader_source);
    let tex = send_to_gpu_textures_next_piece_panel(atlas);
    let meshes = create_geometry_next_piece_panel(atlas);
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

struct NextPiecePanelSpec<'a> {
    piece: GooglyBlockPiece,
    atlas: &'a TextureAtlas2D,
}

fn load_next_piece_panel(
    game: &mut glh::GLState,
    spec: NextPiecePanelSpec, uniforms: &PieceUniformsData) -> NextPiecePanel {
    
    let buffer = create_next_piece_panel_buffer(game, spec.atlas, uniforms);
    NextPiecePanel {
        current_piece: spec.piece,
        buffer: buffer,
    }
}


























#[derive(Copy, Clone)]
struct PlayingFieldBackgroundBuffers {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

#[derive(Copy, Clone)]
struct PlayingFieldBackgroundBufferHandle {
    sp: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
    tex: GLuint,  
}

#[derive(Copy, Clone)]
struct PlayingFieldBackgroundHandle {
    default: PlayingFieldBackgroundBufferHandle,
    dark: PlayingFieldBackgroundBufferHandle,
    light: PlayingFieldBackgroundBufferHandle,
}

#[derive(Copy, Clone)]
struct PlayingFieldBackgroundPanel {
    height: usize,
    width: usize,
    handle: PlayingFieldBackgroundHandle,
}

struct PlayingFieldBackgroundUniforms {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_shaders_playing_field_background() -> ShaderSource {
    let vert_source = include_shader!("playing_field_background.vert.glsl");
    let frag_source = include_shader!("playing_field_background.frag.glsl");

    ShaderSource { 
        vert_name: "playing_field_background.vert.glsl",
        vert_source: vert_source, 
        frag_name: "playing_field_background.frag.glsl",
        frag_source: frag_source 
    }
}

fn create_geometry_playing_field_background(elem: &str, atlas: &TextureAtlas2D) -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [1_f32, 1_f32], [-1_f32, -1_f32], [1_f32, -1_f32],
        [1_f32, 1_f32], [-1_f32,  1_f32], [-1_f32, -1_f32],
    ];
    let corners = atlas.get_name_corners_uv(elem).unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        top_right, bottom_left, bottom_right,
        top_right, top_left, bottom_left
    ];

    ObjMesh::new(points, tex_coords)
}

fn send_to_gpu_shaders_playing_field_background(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_buffers_geometry_playing_field_background() -> PlayingFieldBackgroundBuffers {
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

    PlayingFieldBackgroundBuffers {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_playing_field_background(handle: PlayingFieldBackgroundBuffers, mesh: &ObjMesh) {
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

fn send_to_gpu_textures_playing_field_background(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct PlayingFieldBackgroundSpec<'a> { 
    height: usize, 
    width: usize,
    atlas: &'a TextureAtlas2D,
}

fn load_playing_field_background(game: &mut glh::GLState, spec: PlayingFieldBackgroundSpec) -> PlayingFieldBackgroundPanel {
    let shader_source = create_shaders_playing_field_background();
    let default_mesh = create_geometry_playing_field_background(
        "PlayingFieldDefaultBackground", &spec.atlas
    );
    let dark_mesh = create_geometry_playing_field_background(
        "PlayingFieldFlashingBackgroundDark", &spec.atlas
    );
    let light_mesh = create_geometry_playing_field_background(
        "PlayingFieldFlashingBackgroundLight", &spec.atlas
    );
    let sp = send_to_gpu_shaders_playing_field_background(game, shader_source);
    let default_buffer = create_buffers_geometry_playing_field_background();
    let dark_buffer = create_buffers_geometry_playing_field_background();
    let light_buffer = create_buffers_geometry_playing_field_background();
    send_to_gpu_geometry_playing_field_background(default_buffer, &default_mesh);
    send_to_gpu_geometry_playing_field_background(dark_buffer, &dark_mesh);
    send_to_gpu_geometry_playing_field_background(light_buffer, &light_mesh);
    let tex = send_to_gpu_textures_playing_field_background(&spec.atlas);  
    let v_pos_loc = default_buffer.v_pos_loc;
    let v_tex_loc = default_buffer.v_tex_loc;
    let default_handle = PlayingFieldBackgroundBufferHandle {
        sp: sp,
        vao: default_buffer.vao,
        v_pos_vbo: default_buffer.v_pos_vbo,
        v_tex_vbo: default_buffer.v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
        tex: tex,
    };
    let dark_handle = PlayingFieldBackgroundBufferHandle {
        sp: sp,
        vao: dark_buffer.vao,
        v_pos_vbo: dark_buffer.v_pos_vbo,
        v_tex_vbo: dark_buffer.v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
        tex: tex,
    };
    let light_handle = PlayingFieldBackgroundBufferHandle {
        sp: sp,
        vao: light_buffer.vao,
        v_pos_vbo: light_buffer.v_pos_vbo,
        v_tex_vbo: light_buffer.v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
        tex: tex,
    };
    let handle = PlayingFieldBackgroundHandle {
        default: default_handle,
        dark: dark_handle,
        light: light_handle,
    };

    PlayingFieldBackgroundPanel {
        height: spec.height,
        width: spec.width,
        handle: handle,
    }

}

fn send_to_gpu_uniforms_playing_field_background(sp: GLuint, uniforms: PlayingFieldBackgroundUniforms) {
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





















#[derive(Copy, Clone)]
struct GameOverPanelBuffers {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
}

#[derive(Copy, Clone)]
struct GameOverPanelHandle {
    sp: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    v_pos_loc: GLuint,
    v_tex_loc: GLuint,
    tex: GLuint,
}

#[derive(Copy, Clone)]
struct GameOverPanel {
    height: usize,
    width: usize,
    buffer: GameOverPanelHandle,
}

struct GameOverPanelUniforms {
    gui_scale_mat: Matrix4,
    trans_mat: Matrix4,
}

fn create_shaders_game_over() -> ShaderSource {
    let vert_source = include_shader!("game_over.vert.glsl");
    let frag_source = include_shader!("game_over.frag.glsl");

    ShaderSource { 
        vert_name: "game_over.vert.glsl",
        vert_source: vert_source, 
        frag_name: "game_over.frag.glsl",
        frag_source: frag_source 
    }
}

fn create_geometry_game_over(atlas: &TextureAtlas2D) -> ObjMesh {
    let points: Vec<[f32; 2]> = vec![
        [1_f32, 1_f32], [-1_f32, -1_f32], [1_f32, -1_f32],
        [1_f32, 1_f32], [-1_f32,  1_f32], [-1_f32, -1_f32],
    ];
    let corners = atlas.get_name_corners_uv("GameOver").unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let top_right = [corners.top_right.u, corners.top_right.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let tex_coords: Vec<[f32; 2]> = vec![
        top_right, bottom_left, bottom_right,
        top_right, top_left, bottom_left
    ];

    ObjMesh::new(points, tex_coords)
}

fn send_to_gpu_shaders_game_over(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn create_buffers_geometry_game_over() -> GameOverPanelBuffers {
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

    GameOverPanelBuffers {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        v_pos_loc: v_pos_loc,
        v_tex_loc: v_tex_loc,
    }
}

fn send_to_gpu_geometry_game_over(handle: GameOverPanelBuffers, mesh: &ObjMesh) {
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

fn send_to_gpu_textures_game_over(atlas: &TextureAtlas2D) -> GLuint {
    send_to_gpu_texture(atlas, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct GameOverPanelSpec<'a> { 
    height: usize, 
    width: usize,
    atlas: &'a TextureAtlas2D,
}

fn load_game_over_panel(game: &mut glh::GLState, spec: GameOverPanelSpec) -> GameOverPanel {
    let shader_source = create_shaders_game_over();
    let mesh = create_geometry_game_over(&spec.atlas);
    let sp = send_to_gpu_shaders_game_over(game, shader_source);
    let handle = create_buffers_geometry_game_over();
    send_to_gpu_geometry_game_over(handle, &mesh);
    let tex = send_to_gpu_textures_game_over(&spec.atlas);
    let buffer = GameOverPanelHandle {
        sp: sp,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
        v_pos_loc: handle.v_pos_loc,
        v_tex_loc: handle.v_tex_loc,
        vao: handle.vao,
        tex: tex,
    };

    GameOverPanel {
        buffer: buffer,
        height: spec.height,
        width: spec.width,
    }
}

fn send_to_gpu_uniforms_game_over_panel(sp: GLuint, uniforms: GameOverPanelUniforms) {
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

struct GooglyBlockElementTextureAtlas {
    atlas: TextureAtlas2D,
    coords: HashMap<GooglyBlockElement, TextureQuad>,
}

impl GooglyBlockElementTextureAtlas {
    fn new(atlas: TextureAtlas2D, coords: HashMap<GooglyBlockElement, TextureQuad>) -> GooglyBlockElementTextureAtlas {
        GooglyBlockElementTextureAtlas {
            atlas: atlas,
            coords: coords,
        }
    }
}

fn generate_quad(atlas: &TextureAtlas2D, name: &str) -> TextureQuad {
    let corners = atlas.get_name_corners_uv(name).unwrap();
    let top_left = [corners.top_left.u, corners.top_left.v];
    let bottom_left = [corners.bottom_left.u, corners.bottom_left.v];
    let bottom_right = [corners.bottom_right.u, corners.bottom_right.v];
    let top_right = [corners.top_right.u, corners.top_right.v];

    TextureQuad::new(top_left, bottom_left, bottom_right, top_right)
}

fn create_textures_playing_field(atlas: &TextureAtlas2D) -> GooglyBlockElementTextureAtlas {
    let tex_coords = [
        (GooglyBlockElement::EmptySpace, generate_quad(atlas, "empty_space")),
        (GooglyBlockElement::T, generate_quad(atlas, "t_piece")),
        (GooglyBlockElement::J, generate_quad(atlas, "j_piece")),
        (GooglyBlockElement::Z, generate_quad(atlas, "z_piece")),
        (GooglyBlockElement::O, generate_quad(atlas, "o_piece")),
        (GooglyBlockElement::S, generate_quad(atlas, "s_piece")),
        (GooglyBlockElement::L, generate_quad(atlas, "l_piece")),
        (GooglyBlockElement::I, generate_quad(atlas, "i_piece"))
    ].iter().map(|elem| *elem).collect();
    GooglyBlockElementTextureAtlas::new(atlas.clone(), tex_coords)
}

fn send_to_gpu_textures_playing_field(atlas: &GooglyBlockElementTextureAtlas) -> GLuint {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
        gl::ActiveTexture(gl::TEXTURE0);
        gl::BindTexture(gl::TEXTURE_2D, tex);
        gl::TexImage2D(
            gl::TEXTURE_2D, 0, gl::RGBA as i32, atlas.atlas.width as i32, atlas.atlas.height as i32, 0,
            gl::RGBA, gl::UNSIGNED_BYTE,
            atlas.atlas.as_ptr() as *const GLvoid
        );
        gl::GenerateMipmap(gl::TEXTURE_2D);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as GLint);
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

    tex
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

struct PlayingFieldHandleSpec<'a> {
    rows: usize,
    columns: usize,
    atlas: &'a GooglyBlockElementTextureAtlas,
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
    handle: PlayingFieldHandle,
    atlas: HashMap<GooglyBlockElement, TextureQuad>,
}

impl PlayingField {
    fn new(handle: PlayingFieldHandle, atlas: &GooglyBlockElementTextureAtlas) -> PlayingField {
        let quad = TextureQuad::new([0_f32, 0_f32], [0_f32, 0_f32], [0_f32, 0_f32], [0_f32, 0_f32]);
        PlayingField {
            tex_coords: [[quad; 10]; 20],
            handle: handle,
            atlas: atlas.coords.clone(),
        }
    }

    fn write(&mut self, playing_field: &PlayingFieldState) -> io::Result<usize> {
        let rows = playing_field.landed_blocks.rows();
        let columns = playing_field.landed_blocks.columns();
        for row in 0..rows {
            for column in 0..columns {
                let element = playing_field.landed_blocks.get(row as isize, column as isize).unwrap();
                let quad = self.atlas[&element];
                self.tex_coords[row][column] = quad;
            } 
        }

        let shape = playing_field.current_block.shape();
        let top_left_row = playing_field.current_position.row;
        let top_left_column = playing_field.current_position.column;
        let quad = self.atlas[&shape.element];
        for (shape_row, shape_column) in shape.iter() {
            let row = top_left_row + shape_row as isize;
            let column = top_left_column + shape_column as isize;
            if row >= 0 && column >= 0 {
                self.tex_coords[row as usize][column as usize] = quad;
            }
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
    let sp = send_to_gpu_shaders_playing_field(game, shader_source);
    let handle = create_buffers_geometry_playing_field();
    send_to_gpu_geometry_playing_field(handle, &mesh);
    let tex = send_to_gpu_textures_playing_field(&spec.atlas);
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
    clearing_interval: Interval,
    flash_switch_interval: Interval,
    flash_stop_interval: Interval,
}

struct PlayingFieldTimers {
    fall_timer: Timer,
    collision_timer: Timer,
    left_hold_timer: Timer,
    right_hold_timer: Timer,
    down_hold_timer: Timer,
    rotate_timer: Timer,
    clearing_timer: Timer,
    flash_switch_timer: Timer,
    flash_stop_timer: Timer,
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
            clearing_timer: Timer::new(spec.clearing_interval),
            flash_switch_timer: Timer::new(spec.flash_switch_interval),
            flash_stop_timer: Timer::new(spec.flash_stop_interval),
        }
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

struct ScoreBoard {
    score: usize,
    level: usize,
    lines: usize,
    tetrises: usize,
    lines_before_next_level: usize,
}

impl ScoreBoard {
    fn new() -> ScoreBoard {
        ScoreBoard {
            score: 0,
            level: 0,
            lines: 0,
            tetrises: 0,
            lines_before_next_level: 20,
        }
    }

    fn update(&mut self, new_lines_cleared: usize) {
        self.lines += new_lines_cleared;
        if new_lines_cleared > self.lines_before_next_level {
            self.level += 1;
            self.lines_before_next_level = 20;
        } else {
            self.lines_before_next_level -= new_lines_cleared;
        }

        match new_lines_cleared {
            0 => {}
            1 => {
                self.score += 40;
            }
            2 => {
                self.score += 100;
            }
            3 => {
                self.score += 300;
            }
            _ => {
                self.score += 1200;
                self.tetrises += 1;
            }
        }
    }
}

struct FullRows {
    rows: [isize; 20],
    count: usize,
}

impl FullRows {
    fn new() -> FullRows {
        FullRows {
            rows: [-1; 20],
            count: 0,
        }
    }

    fn clear(&mut self) {
        for i in 0..self.rows.len() {
            self.rows[i] = -1;
        }
        self.count = 0;
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
    StartGame,
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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum FlashAnimationState {
    Light,
    Dark,
    Disabled,
}

struct FlashAnimationStateMachine {
    state: FlashAnimationState,
}

impl FlashAnimationStateMachine {
    fn new() -> FlashAnimationStateMachine {
        FlashAnimationStateMachine {
            state: FlashAnimationState::Disabled,
        }
    }

    #[inline]
    fn is_enabled(&self) -> bool {
        self.state != FlashAnimationState::Disabled
    }

    #[inline]
    fn enable(&mut self) {
        self.state = FlashAnimationState::Dark;
    }

    #[inline]
    fn disable(&mut self) {
        self.state = FlashAnimationState::Disabled;
    }

    #[inline]
    fn is_disabled(&self) -> bool {
        self.state == FlashAnimationState::Disabled
    }

    #[inline]
    fn update(&mut self) {
        self.state = match self.state {
            FlashAnimationState::Disabled => FlashAnimationState::Disabled,
            FlashAnimationState::Dark => FlashAnimationState::Light,
            FlashAnimationState::Light => FlashAnimationState::Dark,
        };
    }
}




















#[derive(Clone)]
struct TitleScreenStateMachineSpec {
    transition_interval: Interval,
    pressed_interval: Interval,
    unpressed_interval: Interval,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TitleScreenBlinkState {
    Disabled,
    Unpressed,
    Pressed,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TitleScreenAnimationState {
    Disabled,
    On,
    Off,
}

struct TitleScreenBlinkStateMachine {
    state: TitleScreenBlinkState,
    animation_state: TitleScreenAnimationState,
    unpressed_blink_timer: Timer,
    pressed_blink_timer: Timer,
}

impl TitleScreenBlinkStateMachine {
    fn new(spec: &TitleScreenStateMachineSpec) -> TitleScreenBlinkStateMachine {
        TitleScreenBlinkStateMachine {
            state: TitleScreenBlinkState::Disabled,
            animation_state: TitleScreenAnimationState::Disabled,
            unpressed_blink_timer: Timer::new(spec.unpressed_interval),
            pressed_blink_timer: Timer::new(spec.pressed_interval),
        }
    }

    #[inline]
    fn is_enabled(&self) -> bool {
        self.state != TitleScreenBlinkState::Disabled
    }

    #[inline]
    fn enable(&mut self) {
        if self.state == TitleScreenBlinkState::Disabled {
            self.state = TitleScreenBlinkState::Unpressed;
            self.animation_state = TitleScreenAnimationState::On;
            self.unpressed_blink_timer.reset();
            self.pressed_blink_timer.reset();
        }
    }

    #[inline]
    fn disable(&mut self) {
        self.state = TitleScreenBlinkState::Disabled;
        self.animation_state = TitleScreenAnimationState::Disabled;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    fn is_disabled(&self) -> bool {
        self.state == TitleScreenBlinkState::Disabled
    }

    #[inline]
    fn unpressed(&mut self) {
        self.state = TitleScreenBlinkState::Unpressed;
        self.animation_state = TitleScreenAnimationState::On;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    fn is_unpressed(&self) -> bool {
        self.state == TitleScreenBlinkState::Unpressed
    }

    #[inline]
    fn pressed(&mut self) {
        self.state = TitleScreenBlinkState::Pressed;
        self.animation_state = TitleScreenAnimationState::On;
        self.unpressed_blink_timer.reset();
        self.pressed_blink_timer.reset();
    }

    #[inline]
    fn is_pressed(&self) -> bool {
        self.state == TitleScreenBlinkState::Pressed
    }

    #[inline]
    fn animation_is_on(&self) -> bool {
        self.animation_state == TitleScreenAnimationState::On
    }

    #[inline]
    fn update(&mut self, elapsed_milliseconds: Duration) {
        match self.state {
            TitleScreenBlinkState::Disabled => {}
            TitleScreenBlinkState::Unpressed => {
                self.pressed_blink_timer.reset();
                self.unpressed_blink_timer.update(elapsed_milliseconds);
                if self.unpressed_blink_timer.event_triggered() {
                    self.animation_state = match self.animation_state {
                        TitleScreenAnimationState::On => TitleScreenAnimationState::Off,
                        TitleScreenAnimationState::Off => TitleScreenAnimationState::On,
                        TitleScreenAnimationState::Disabled => TitleScreenAnimationState::Disabled,
                    };
                    self.unpressed_blink_timer.reset();
                }
            }
            TitleScreenBlinkState::Pressed => {
                self.unpressed_blink_timer.reset();
                self.pressed_blink_timer.update(elapsed_milliseconds);
                if self.pressed_blink_timer.event_triggered() {
                    self.animation_state = match self.animation_state {
                        TitleScreenAnimationState::On => TitleScreenAnimationState::Off,
                        TitleScreenAnimationState::Off => TitleScreenAnimationState::On,
                        TitleScreenAnimationState::Disabled => TitleScreenAnimationState::Disabled,
                    };
                    self.pressed_blink_timer.reset();
                }
            }
        }
    }
}

struct TitleScreenStateMachine {
    blink_state: TitleScreenBlinkStateMachine,
    transition_timer: Timer,
}

impl TitleScreenStateMachine {
    fn new(spec: TitleScreenStateMachineSpec) -> TitleScreenStateMachine {
        let mut blink_state = TitleScreenBlinkStateMachine::new(&spec);
        blink_state.unpressed(); 
        TitleScreenStateMachine {
            blink_state: blink_state,
            transition_timer: Timer::new(spec.transition_interval),
        }
    }
    
    #[inline]
    fn animation_is_on(&self) -> bool {
        self.blink_state.animation_is_on()
    }

    #[inline]
    fn is_pressed(&self) -> bool {
        self.blink_state.is_pressed()
    }
}



















#[derive(Copy, Clone)]
struct GameTitleScreenState {}

impl GameTitleScreenState {
    fn new() -> GameTitleScreenState {
        GameTitleScreenState {}
    }

    fn handle_input(&self, context: &mut GameContext, input: Input, elapsed_milliseconds: Duration) {
        let mut title_screen = context.title_screen.borrow_mut();
        match input.kind {
            InputKind::StartGame => {
                match input.action {
                    InputAction::Press | InputAction::Repeat => {
                        title_screen.blink_state.pressed();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn update(&self, context: &mut GameContext, elapsed_milliseconds: Duration) -> GameState {
        let mut title_screen = context.title_screen.borrow_mut();
        if title_screen.blink_state.is_disabled() {
            title_screen.blink_state.enable();
        }

        if title_screen.blink_state.is_pressed() {    
            title_screen.transition_timer.update(elapsed_milliseconds);
            if title_screen.transition_timer.event_triggered() {
                title_screen.blink_state.disable();
                return GameState::Falling(GameFallingState::new());
            }
        }

        title_screen.blink_state.update(elapsed_milliseconds);
        GameState::TitleScreen(*self)
    }
}

#[derive(Copy, Clone)]
struct GameFallingState {}

impl GameFallingState {
    fn new() -> GameFallingState {
        GameFallingState {}
    }

    fn handle_input(&mut self, context: &mut GameContext, input: Input, elapsed_milliseconds: Duration) {
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
            InputKind::Exit => {
                context.exiting = true;
            }
            _ => {}
        } 
    }

    fn update(&mut self, context: &mut GameContext, elapsed_milliseconds: Duration) -> GameState {
        if context.exiting {
            return GameState::Exiting(GameExitingState::new());
        }
        
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        let mut statistics = context.statistics.borrow_mut();
        let mut next_block = context.next_block.borrow_mut();
        let mut full_rows = context.full_rows.borrow_mut();
        let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();

        let collides_with_floor = playing_field_state.collides_with_floor_below();
        let collides_with_element = playing_field_state.collides_with_element_below();

        timers.fall_timer.update(elapsed_milliseconds);
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
            if !playing_field_state.has_empty_row(0) {
                return GameState::GameOver(GameGameOverState::new());
            }
            
            statistics.update(current_block);
            let old_next_block = next_block.block;
            next_block.update();
            let new_next_block = GooglyBlock::new(old_next_block, GooglyBlockRotation::R0);
            playing_field_state.update_new_block(new_next_block);
            timers.collision_timer.reset();
        }
        
        if flashing_state_machine.is_enabled() {
            timers.flash_switch_timer.update(elapsed_milliseconds);
            timers.flash_stop_timer.update(elapsed_milliseconds);
            if timers.flash_stop_timer.event_triggered() {
                timers.flash_switch_timer.reset();
                timers.flash_stop_timer.reset();
                flashing_state_machine.disable();
            } else if timers.flash_switch_timer.event_triggered() {
                timers.flash_switch_timer.reset();
                flashing_state_machine.update();
            }
        }

        let full_row_count = playing_field_state.get_full_rows(&mut full_rows.rows);
        full_rows.count = full_row_count;
        if full_row_count > 0 {
            if full_row_count >= 4 {
                flashing_state_machine.enable();
            }
            return GameState::Clearing(GameClearingState::new());
        } else {
            return GameState::Falling(GameFallingState::new());
        }
    }
}

#[derive(Copy, Clone)]
struct GameClearingState {
    columns_cleared: usize,
}

impl GameClearingState {
    fn new() -> GameClearingState {
        GameClearingState {
            columns_cleared: 0,
        }
    }

    fn handle_input(&mut self, context: &mut GameContext, input: Input, elapsed_milliseconds: Duration) {
        match input.kind {
            InputKind::Exit => {
                context.exiting = true;
            }
            _ => {}
        }
    }

    fn update(&mut self, context: &mut GameContext, elapsed_milliseconds: Duration) -> GameState {
        let mut timers = context.timers.borrow_mut();
        let mut playing_field_state = context.playing_field_state.borrow_mut();
        let mut full_rows = context.full_rows.borrow_mut();
        let mut score_board = context.score_board.borrow_mut();
        let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();
        
        timers.clearing_timer.update(elapsed_milliseconds);
        if timers.clearing_timer.event_triggered() {
            timers.clearing_timer.reset();
            let center_left = (4 - self.columns_cleared / 2) as isize;
            let center_right = (5 + self.columns_cleared / 2) as isize;
            for row in full_rows.rows.iter() {
                if *row >= 0 {
                    playing_field_state.landed_blocks.clear(*row, center_left);
                    playing_field_state.landed_blocks.clear(*row, center_right);
                }
            }
            self.columns_cleared += 2;
        }

        if flashing_state_machine.is_enabled() {
            timers.flash_switch_timer.update(elapsed_milliseconds);
            timers.flash_stop_timer.update(elapsed_milliseconds);
            if timers.flash_stop_timer.event_triggered() {
                timers.flash_switch_timer.reset();
                timers.flash_stop_timer.reset();
                flashing_state_machine.disable();
            } else if timers.flash_switch_timer.event_triggered() {
                timers.flash_switch_timer.reset();
                flashing_state_machine.update();
            }
        }

        if self.columns_cleared >= 10 {
            playing_field_state.collapse_empty_rows();
            score_board.update(full_rows.count);
            full_rows.clear();
            self.columns_cleared = 0;

            return GameState::Falling(GameFallingState::new());
        }

        GameState::Clearing(self.clone())
    }
}

#[derive(Copy, Clone)]
struct GameGameOverState {}

impl GameGameOverState {
    fn new() -> GameGameOverState {
        GameGameOverState {}
    }

    fn handle_input(&mut self, context: &mut GameContext, input: Input, elapsed_milliseconds: Duration) {
        match input.kind {
            InputKind::Exit => {
                context.exiting = true;
            }
            _ => {}
        }
    }

    fn update(&mut self, context: &mut GameContext, elapsed_milliseconds: Duration) -> GameState {
        if context.exiting {
            GameState::Exiting(GameExitingState::new())
        } else {
            let mut flashing_state_machine = context.flashing_state_machine.borrow_mut();
            flashing_state_machine.disable();

            GameState::GameOver(*self)
        }
    }
}

#[derive(Copy, Clone)]
struct GameExitingState {}

impl GameExitingState {
    fn new() -> GameExitingState { 
        GameExitingState {} 
    }

    fn handle_input(&mut self, context: &mut GameContext, input: Input, elapsed_milliseconds: Duration) {
        match input.kind {
            _ => {}
        }
    }

    fn update(&mut self, context: &mut GameContext, elapsed_milliseconds: Duration) -> GameState {
        context.gl.borrow_mut().window.set_should_close(true);
        GameState::Exiting(*self)
    }
}

#[derive(Copy, Clone)]
enum GameState {
    TitleScreen(GameTitleScreenState),
    Falling(GameFallingState),
    Clearing(GameClearingState),
    GameOver(GameGameOverState),
    Exiting(GameExitingState),
}

struct GameStateMachine {
    context: Rc<RefCell<GameContext>>,
    state: GameState,
}

impl GameStateMachine {
    fn new(context: Rc<RefCell<GameContext>>, initial_state: GameState) -> GameStateMachine {
        GameStateMachine {
            context: context,
            state: initial_state,
        }
    }

    fn handle_input(&mut self, input: Input, elapsed_milliseconds: Duration) {
        let mut context = self.context.borrow_mut();
        match self.state {
            GameState::TitleScreen(s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            GameState::Falling(mut s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            GameState::Clearing(mut s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            GameState::GameOver(mut s) => s.handle_input(&mut context, input, elapsed_milliseconds),
            GameState::Exiting(mut s) => s.handle_input(&mut context, input, elapsed_milliseconds),
        }
    }

    fn update(&mut self, elapsed_milliseconds: Duration) -> GameState {
        let mut context = self.context.borrow_mut();
        self.state = match self.state {
            GameState::TitleScreen(s) => s.update(&mut context, elapsed_milliseconds),
            GameState::Falling(mut s) => s.update(&mut context, elapsed_milliseconds),
            GameState::Clearing(mut s) => s.update(&mut context, elapsed_milliseconds),
            GameState::GameOver(mut s) => s.update(&mut context, elapsed_milliseconds),
            GameState::Exiting(mut s) => s.update(&mut context, elapsed_milliseconds),
        };

        self.state
    }
}

struct NextBlockGen {
    rng: rng::ThreadRng,
    between: Uniform<u32>,
    last_block: GooglyBlockPiece,
    table: [GooglyBlockPiece; 7],
}

impl NextBlockGen {
    fn new() -> NextBlockGen {
        let table = [
            GooglyBlockPiece::T,
            GooglyBlockPiece::J,
            GooglyBlockPiece::Z,
            GooglyBlockPiece::O,
            GooglyBlockPiece::S,
            GooglyBlockPiece::L,
            GooglyBlockPiece::I,
        ];
        let mut rng = rng::thread_rng();
        let between = Uniform::new_inclusive(0, 6);
        let random = between.sample(&mut rng) as usize;
        let last_block = table[random];

        NextBlockGen {
            rng: rng,
            between: between,
            last_block: last_block,
            table: table,
        }
    }

    fn next(&mut self) -> GooglyBlockPiece {
        let mut block = self.table[self.between.sample(&mut self.rng) as usize];
        let mut gas = 0;
        while (gas < 8) && (block == self.last_block) {
            let random = self.between.sample(&mut self.rng) as usize;
            block = self.table[random];
            gas += 1;
        }
        self.last_block = block;
        
        block
    }
}

struct NextBlockCell {
    gen: NextBlockGen,
    block: GooglyBlockPiece,
}

impl NextBlockCell {
    fn new() -> NextBlockCell {
        let mut gen = NextBlockGen::new();
        let block = gen.next();
        
        NextBlockCell {
            gen: gen,
            block: block,
        }
    }

    fn update(&mut self) {
        self.block = self.gen.next();
    }
}

struct GameContext {
    gl: Rc<RefCell<glh::GLState>>,
    timers: Rc<RefCell<PlayingFieldTimers>>,
    playing_field_state: Rc<RefCell<PlayingFieldState>>,
    next_block: Rc<RefCell<NextBlockCell>>,
    statistics: Rc<RefCell<Statistics>>,
    score_board: Rc<RefCell<ScoreBoard>>,
    full_rows: Rc<RefCell<FullRows>>,
    flashing_state_machine: Rc<RefCell<FlashAnimationStateMachine>>,
    exiting: bool,
    title_screen: Rc<RefCell<TitleScreenStateMachine>>,
}

struct RendererContext {
    game_context: Rc<RefCell<GameContext>>,
    title_screen: TitleScreenHandle,
    playing_field: PlayingField,
    ui: UI,
    background: BackgroundPanel,
    playing_field_background: PlayingFieldBackgroundPanel,
    game_over: GameOverPanel,
}

impl RendererContext {
    #[inline]
    fn get_framebuffer_size(&self) -> (i32, i32) {
        self.game_context.borrow().gl.borrow_mut().window.get_framebuffer_size()
    }

    #[inline]
    fn viewport_dimensions(&self) -> ViewportDimensions {
        let (width, height) = {
            let game_context = self.game_context.borrow();
            let gl_context = game_context.gl.borrow();
            (gl_context.width as i32, gl_context.height as i32)
        };
        
        ViewportDimensions { 
            width: width, 
            height: height,
        }
    }

    fn update_uniforms_background_panel(&mut self) {
        let panel_width = self.background.width as f32;
        let panel_height = self.background.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0_f32);
        let uniforms = BackgroundPanelUniforms { gui_scale_mat: gui_scale_mat };
        send_to_gpu_uniforms_background_panel(self.background.background_handle.sp, uniforms);
    }

    fn update_uniforms_title_background_panel(&mut self) {
        let panel_width = self.background.width as f32;
        let panel_height = self.background.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0_f32);
        let uniforms = BackgroundPanelUniforms { gui_scale_mat: gui_scale_mat };
        send_to_gpu_uniforms_background_panel(self.background.background_handle.sp, uniforms);        
    }

    fn update_uniforms_ui_panel(&mut self) {
        let panel_width = self.ui.ui_panel.width as f32;
        let panel_height = self.ui.ui_panel.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0_f32);
        let trans_mat = Matrix4::one();
        let uniforms = UIPanelUniforms { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
        send_to_gpu_uniforms_ui_panel(self.ui.ui_panel.sp, uniforms);
    }

    fn update_uniforms_next_piece_panel(&mut self) {
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let scale = 50;
        // FIXME: MAGIC NUMBERS IN USE.
        let gui_scale_x = 2.0 * (scale as f32) / (viewport_width as f32);
        let gui_scale_y = 2.0 * (scale as f32) / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
        let trans_mat = match self.game_context.borrow().next_block.borrow().block {
            GooglyBlockPiece::T => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
            GooglyBlockPiece::J => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
            GooglyBlockPiece::Z => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
            GooglyBlockPiece::O => Matrix4::from_translation(cgmath::vec3((0.50, 0.43, 0.0))),
            GooglyBlockPiece::S => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
            GooglyBlockPiece::L => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
            GooglyBlockPiece::I => Matrix4::from_translation(cgmath::vec3((0.555, 0.48, 0.0))),
        };
        let uniforms = PieceUniformsData { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
        send_to_gpu_uniforms_next_piece_panel(self.ui.next_piece_panel.buffer.sp, &uniforms);
    }

    fn update_uniforms_playing_field(&mut self) {
        let viewport = self.viewport_dimensions();
        let scale = 488;
        let gui_scale_x = (scale as f32) / (viewport.width as f32);
        let gui_scale_y = (scale as f32) / (viewport.height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
        let trans_mat = Matrix4::from_translation(cgmath::vec3((0.085, 0.0, 0.0)));
        let uniforms = PlayingFieldUniforms { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
        send_to_gpu_uniforms_playing_field(self.ui.next_piece_panel.buffer.sp, uniforms);
    }

    fn update_uniforms_game_over_panel(&mut self) {
        let panel_width = self.game_over.width as f32;
        let panel_height = self.game_over.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0.0);
        let trans_mat = Matrix4::from_translation(cgmath::vec3((0.08, 0.0, 0.0)));
        let uniforms = GameOverPanelUniforms { 
            gui_scale_mat: gui_scale_mat,
            trans_mat: trans_mat,
        };
        send_to_gpu_uniforms_game_over_panel(self.game_over.buffer.sp, uniforms);
    }

    fn update_uniforms_playing_field_background(&mut self) {
        let panel_width = self.playing_field_background.width as f32;
        let panel_height = self.playing_field_background.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0.0);
        let trans_mat = Matrix4::from_translation(cgmath::vec3((0.08, 0.0, 0.0)));
        let uniforms = PlayingFieldBackgroundUniforms { 
            gui_scale_mat: gui_scale_mat,
            trans_mat: trans_mat,
        };
        let sp = self.playing_field_background.handle.default.sp;
        send_to_gpu_uniforms_playing_field_background(sp, uniforms);
    }

    fn update_uniforms_title_screen_background(&mut self) {
        let panel_width = self.title_screen.background_handle.width as f32;
        let panel_height = self.title_screen.background_handle.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0.0);
        let trans_mat = Matrix4::from_translation(cgmath::vec3((0.0, 0.0, 0.0)));
        let uniforms = TitleScreenBackgroundUniforms { 
            gui_scale_mat: gui_scale_mat,
            trans_mat: trans_mat,
        };
        let sp = self.title_screen.background_handle.handle.sp;
        send_to_gpu_uniforms_title_screen_background(sp, uniforms);
    }

    fn update_uniforms_title_screen_flashing(&mut self) {
        let panel_width = self.title_screen.flashing_handle.width as f32;
        let panel_height = self.title_screen.flashing_handle.height as f32;
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let gui_scale_x = panel_width / (viewport_width as f32);
        let gui_scale_y = panel_height / (viewport_height as f32);
        let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0.0);
        let placement = self.title_screen.flashing_handle.placement;
        let trans_mat = Matrix4::from_translation(cgmath::vec3((placement.x, placement.y, 0.0)));
        let uniforms = TitleScreenFlashingUniforms { 
            gui_scale_mat: gui_scale_mat,
            trans_mat: trans_mat,
        };
        let sp = self.title_screen.flashing_handle.handle.sp;
        send_to_gpu_uniforms_title_screen_flashing(sp, uniforms);
    }
}

#[derive(Copy, Clone)]
struct RendererTitleScreenState {}

impl RendererTitleScreenState {
    fn update_uniforms_background(&self, context: &mut RendererContext) {
        context.update_uniforms_title_screen_background();
    }

    fn render_background(&self, context: &mut RendererContext) {
        let handle = context.title_screen.background_handle.handle;
        unsafe {
            gl::UseProgram(handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, handle.tex);
            gl::BindVertexArray(handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn update_uniforms_start_prompt(&self, context: &mut RendererContext) {
        context.update_uniforms_title_screen_flashing();
    }

    fn render_start_prompt(&self, context: &mut RendererContext) {
        let game_context = context.game_context.borrow();
        // If the text prompt animation is in the off part of the animation,
        // we do not want to render anything. Otherwise, we display the start prompt.
        // Oscillating between these states is what produces the blinking pattern.
        let game_title_screen = game_context.title_screen.borrow();
        if game_title_screen.animation_is_on() {
            let handle = context.title_screen.flashing_handle.handle;
            unsafe {
                gl::UseProgram(handle.sp);
                gl::Disable(gl::DEPTH_TEST);
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, handle.tex);
                gl::BindVertexArray(handle.vao);
                gl::DrawArrays(gl::TRIANGLES, 0, 6);
                gl::Disable(gl::BLEND);
            }
        }
    }

    fn render(&self, context: &mut RendererContext) {
        self.update_uniforms_background(context);
        self.render_background(context);
        self.update_uniforms_start_prompt(context);
        self.render_start_prompt(context);
    }
}

#[derive(Copy, Clone)]
struct RendererFallingState {}

impl RendererFallingState {
    #[inline]
    fn clear_framebuffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
        }
    }

    #[inline]
    fn clear_depth_buffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
        }
    }

    #[inline]
    fn update_viewport(&self, context: &mut RendererContext) {
        let dims = context.viewport_dimensions();
        unsafe {
            gl::Viewport(0, 0, dims.width, dims.height);
        }
    }

    #[inline]
    fn update_background(&self, context: &mut RendererContext) {
        context.update_uniforms_background_panel();
    }

    #[inline]
    fn update_title_background(&self, context: &mut RendererContext) {
        context.update_uniforms_title_background_panel();
    }

    fn render_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.background_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.background_handle.tex);
            gl::BindVertexArray(context.background.background_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }        
    }

    fn render_title_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.title_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.title_handle.tex);
            gl::BindVertexArray(context.background.title_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn update_ui(&self, context: &mut RendererContext) {
        context.update_uniforms_ui_panel();
        context.update_uniforms_next_piece_panel();
        let game_context = context.game_context.borrow();
        let score_board = game_context.score_board.borrow();
        context.ui.update_score(score_board.score);
        context.ui.update_lines(score_board.lines);
        context.ui.update_level(score_board.level);
        context.ui.update_tetrises(score_board.tetrises);
        context.ui.update_statistics(&game_context.statistics.borrow());
        context.ui.update_next_piece(game_context.next_block.borrow().block);
        context.ui.update_panel();   
    }

    fn render_ui(&self, context: &mut RendererContext) {
        // Render the game board. We turn off depth testing to do so since this is
        // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
        // from rendering the game board.
        unsafe {
            gl::UseProgram(context.ui.ui_panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.ui_panel.tex);
            gl::BindVertexArray(context.ui.ui_panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(context.ui.text_panel.buffer.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.text_panel.buffer.buffer.tex);
            gl::BindVertexArray(context.ui.text_panel.buffer.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 47 * 6);

            gl::UseProgram(context.ui.next_piece_panel.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.next_piece_panel.buffer.tex);
            gl::BindVertexArray(context.ui.next_piece_panel.buffer.handle(context.game_context.borrow().next_block.borrow().block).vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * 8);
            gl::Disable(gl::BLEND);
        }
    }

    fn update_playing_field(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field();
        let game_context = context.game_context.borrow();
        let playing_field_state = game_context.playing_field_state.borrow();
        context.playing_field.write(&playing_field_state).unwrap();
        context.playing_field.send_to_gpu().unwrap();
    }

    fn render_playing_field(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.playing_field.handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.playing_field.handle.tex);
            gl::BindVertexArray(context.playing_field.handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 2 * 6 * 20 * 10);
            gl::Disable(gl::BLEND);
        }        
    }

    fn update_playing_field_background(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field_background();
    }

    fn render_playing_field_background(&self, context: &mut RendererContext) {
        // Check which background image to use by introspecting the game context for the state of the 
        // flashing state machine.
        let game_context = context.game_context.borrow();
        let flashing_state_machine = game_context.flashing_state_machine.borrow();
        let flashing_state_handle = context.playing_field_background.handle;
        let handle = match flashing_state_machine.state {
            FlashAnimationState::Light => flashing_state_handle.light,
            FlashAnimationState::Dark => flashing_state_handle.dark,
            FlashAnimationState::Disabled => flashing_state_handle.default,
        };

        unsafe {
            gl::UseProgram(handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, handle.tex);
            gl::BindVertexArray(handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn render(&self, context: &mut RendererContext) {
        self.clear_framebuffer(context);
        self.clear_depth_buffer(context);
        self.update_viewport(context);
        self.update_background(context);
        self.render_background(context);
        self.update_title_background(context);
        self.render_title_background(context);
        self.update_playing_field_background(context);
        self.render_playing_field_background(context);
        self.update_ui(context);
        self.render_ui(context);
        self.update_playing_field(context);
        self.render_playing_field(context);
    }
}

#[derive(Copy, Clone)]
struct RendererClearingState {}

impl RendererClearingState {
    #[inline]
    fn clear_framebuffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
        }
    }

    #[inline]
    fn clear_depth_buffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
        }
    }

    #[inline]
    fn update_viewport(&self, context: &mut RendererContext) {
        let dims = context.viewport_dimensions();
        unsafe {
            gl::Viewport(0, 0, dims.width, dims.height);
        }
    }

    #[inline]
    fn update_background(&self, context: &mut RendererContext) {
        context.update_uniforms_background_panel();
    }

    #[inline]
    fn update_title_background(&self, context: &mut RendererContext) {
        context.update_uniforms_title_background_panel();
    }

    fn render_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.background_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.background_handle.tex);
            gl::BindVertexArray(context.background.background_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }        
    }

    fn render_title_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.title_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.title_handle.tex);
            gl::BindVertexArray(context.background.title_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);            
        }
    }

    fn update_ui(&self, context: &mut RendererContext) {
        context.update_uniforms_ui_panel();
        context.update_uniforms_next_piece_panel();
        let game_context = context.game_context.borrow();
        let score_board = game_context.score_board.borrow();
        context.ui.update_score(score_board.score);
        context.ui.update_lines(score_board.lines);
        context.ui.update_level(score_board.level);
        context.ui.update_tetrises(score_board.tetrises);
        context.ui.update_statistics(&game_context.statistics.borrow());
        context.ui.update_next_piece(game_context.next_block.borrow().block);
        context.ui.update_panel();   
    }

    fn render_ui(&self, context: &mut RendererContext) {
        // Render the game board. We turn off depth testing to do so since this is
        // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
        // from rendering the game board.
        unsafe {
            gl::UseProgram(context.ui.ui_panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.ui_panel.tex);
            gl::BindVertexArray(context.ui.ui_panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(context.ui.text_panel.buffer.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.text_panel.buffer.buffer.tex);
            gl::BindVertexArray(context.ui.text_panel.buffer.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 47 * 6);

            gl::UseProgram(context.ui.next_piece_panel.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.next_piece_panel.buffer.tex);
            gl::BindVertexArray(context.ui.next_piece_panel.buffer.handle(context.game_context.borrow().next_block.borrow().block).vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * 8);
            gl::Disable(gl::BLEND);
        }
    }

    fn update_playing_field(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field();
        let game_context = context.game_context.borrow();
        let playing_field_state = game_context.playing_field_state.borrow();
        context.playing_field.write(&playing_field_state).unwrap();
        context.playing_field.send_to_gpu().unwrap();
    }

    fn render_playing_field(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.playing_field.handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.playing_field.handle.tex);
            gl::BindVertexArray(context.playing_field.handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 2 * 6 * 20 * 10);
            gl::Disable(gl::BLEND);
        }
    }

    fn update_playing_field_background(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field_background();
    }

    fn render_playing_field_background(&self, context: &mut RendererContext) {
        // Check which background image to use by introspecting the game context for the state of the 
        // flashing state machine.
        let game_context = context.game_context.borrow();
        let flashing_state_machine = game_context.flashing_state_machine.borrow();
        let flashing_state_handle = context.playing_field_background.handle;
        let handle = match flashing_state_machine.state {
            FlashAnimationState::Light => flashing_state_handle.light,
            FlashAnimationState::Dark => flashing_state_handle.dark,
            FlashAnimationState::Disabled => flashing_state_handle.default,
        };

        unsafe {
            gl::UseProgram(handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, handle.tex);
            gl::BindVertexArray(handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn render(&self, context: &mut RendererContext) {
        self.clear_framebuffer(context);
        self.clear_depth_buffer(context);
        self.update_viewport(context);
        self.update_background(context);
        self.render_background(context);
        self.update_title_background(context);
        self.render_title_background(context);
        self.update_playing_field_background(context);
        self.render_playing_field_background(context);
        self.update_ui(context);
        self.render_ui(context);
        self.update_playing_field(context);
        self.render_playing_field(context);
    }
}

#[derive(Copy, Clone)]
struct RendererGameOverState {}

impl RendererGameOverState {
    #[inline]
    fn clear_framebuffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
        }
    }

    #[inline]
    fn clear_depth_buffer(&self, context: &mut RendererContext) {
        unsafe {
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
        }
    }

    #[inline]
    fn update_viewport(&self, context: &mut RendererContext) {
        let dims = context.viewport_dimensions();
        unsafe {
            gl::Viewport(0, 0, dims.width, dims.height);
        }
    }

    #[inline]
    fn update_background(&self, context: &mut RendererContext) {
        context.update_uniforms_background_panel();
    }

    #[inline]
    fn update_title_background(&self, context: &mut RendererContext) {
        context.update_uniforms_title_background_panel();
    }

    fn render_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.background_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.background_handle.tex);
            gl::BindVertexArray(context.background.background_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }        
    }

    fn render_title_background(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.background.title_handle.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.background.title_handle.tex);
            gl::BindVertexArray(context.background.title_handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn update_ui(&self, context: &mut RendererContext) {
        context.update_uniforms_ui_panel();
        context.update_uniforms_next_piece_panel();
        let game_context = context.game_context.borrow();
        let score_board = game_context.score_board.borrow();
        context.ui.update_score(score_board.score);
        context.ui.update_lines(score_board.lines);
        context.ui.update_level(score_board.level);
        context.ui.update_tetrises(score_board.tetrises);
        context.ui.update_statistics(&game_context.statistics.borrow());
        context.ui.update_next_piece(game_context.next_block.borrow().block);
        context.ui.update_panel();   
    }

    fn render_ui(&self, context: &mut RendererContext) {
        // Render the game board. We turn off depth testing to do so since this is
        // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
        // from rendering the game board.
        unsafe {
            gl::UseProgram(context.ui.ui_panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.ui_panel.tex);
            gl::BindVertexArray(context.ui.ui_panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(context.ui.text_panel.buffer.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.text_panel.buffer.buffer.tex);
            gl::BindVertexArray(context.ui.text_panel.buffer.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 47 * 6);

            gl::UseProgram(context.ui.next_piece_panel.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.ui.next_piece_panel.buffer.tex);
            gl::BindVertexArray(context.ui.next_piece_panel.buffer.handle(context.game_context.borrow().next_block.borrow().block).vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * 8);
            gl::Disable(gl::BLEND);
        }
    }

    fn update_playing_field(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field();
        let game_context = context.game_context.borrow();
        let playing_field_state = game_context.playing_field_state.borrow();
        context.playing_field.write(&playing_field_state).unwrap();
        context.playing_field.send_to_gpu().unwrap();
    }

    fn render_playing_field(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.playing_field.handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.playing_field.handle.tex);
            gl::BindVertexArray(context.playing_field.handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 2 * 6 * 20 * 10);
            gl::Disable(gl::BLEND);
        }        
    }

    fn update_game_over_panel(&self, context: &mut RendererContext) {
        context.update_uniforms_game_over_panel();
    }

    fn render_game_over_panel(&self, context: &mut RendererContext) {
        unsafe {
            gl::UseProgram(context.game_over.buffer.sp);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, context.game_over.buffer.tex);
            gl::BindVertexArray(context.game_over.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            gl::Disable(gl::BLEND);
        }  
    }

    fn update_playing_field_background(&self, context: &mut RendererContext) {
        context.update_uniforms_playing_field_background();
    }

    fn render_playing_field_background(&self, context: &mut RendererContext) {
        // Check which background image to use by introspecting the game context for the state of the 
        // flashing state machine.
        let game_context = context.game_context.borrow();
        let flashing_state_machine = game_context.flashing_state_machine.borrow();
        let flashing_state_handle = context.playing_field_background.handle;
        let handle = match flashing_state_machine.state {
            FlashAnimationState::Light => flashing_state_handle.light,
            FlashAnimationState::Dark => flashing_state_handle.dark,
            FlashAnimationState::Disabled => flashing_state_handle.default,
        };

        unsafe {
            gl::UseProgram(handle.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, handle.tex);
            gl::BindVertexArray(handle.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    fn render(&self, context: &mut RendererContext) {
        self.clear_framebuffer(context);
        self.clear_depth_buffer(context);
        self.update_viewport(context);
        self.update_background(context);
        self.render_background(context);
        self.update_title_background(context);
        self.render_title_background(context);
        self.update_playing_field_background(context);
        self.render_playing_field_background(context);
        self.update_ui(context);
        self.render_ui(context);
        self.update_playing_field(context);
        self.render_playing_field(context);
        self.update_game_over_panel(context);
        self.render_game_over_panel(context);
    }
}

#[derive(Copy, Clone)]
struct RendererExitingState {}

impl RendererExitingState {
    fn render(&self, context: &mut RendererContext) {}
}

enum RendererState {
    TitleScreen(RendererTitleScreenState),
    Falling(RendererFallingState),
    Clearing(RendererClearingState),
    GameOver(RendererGameOverState),
    Exiting(RendererExitingState),
}

struct RendererStateMachine {
    context: RendererContext,
    state: RendererState,
}

impl RendererStateMachine {
    fn new(context: RendererContext, initial_state: RendererState) -> RendererStateMachine {
        RendererStateMachine {
            context: context,
            state: initial_state,
        }
    }

    fn update(&mut self, game_state: GameState) {
        self.state = match game_state {
            GameState::TitleScreen(_) => RendererState::TitleScreen(RendererTitleScreenState {}),
            GameState::Falling(_) => RendererState::Falling(RendererFallingState {}),
            GameState::Clearing(_) => RendererState::Clearing(RendererClearingState {}),
            GameState::GameOver(_) => RendererState::GameOver(RendererGameOverState {}),
            GameState::Exiting(_) => RendererState::Exiting(RendererExitingState {}),
        }
    }

    fn render(&mut self) {
        match self.state {
            RendererState::TitleScreen(s) => s.render(&mut self.context),
            RendererState::Falling(s) => s.render(&mut self.context),
            RendererState::Clearing(s) => s.render(&mut self.context),
            RendererState::GameOver(s) => s.render(&mut self.context),
            RendererState::Exiting(s) => s.render(&mut self.context),
        }
    }
}

struct Game {
    context: Rc<RefCell<GameContext>>,
    state_machine: GameStateMachine,
    renderer_state_machine: RendererStateMachine,
}

impl Game {
    #[inline]
    fn window_should_close(&self) -> bool {
        self.context.borrow().gl.borrow_mut().window.should_close()
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
    fn handle_input(&mut self, input: Input, elapsed_milliseconds: Duration) {
        self.state_machine.handle_input(input, elapsed_milliseconds);
    }

    fn update_state(&mut self, elapsed_milliseconds: Duration) {
        let state = self.state_machine.update(elapsed_milliseconds);
        self.renderer_state_machine.update(state);
    }

    fn render(&mut self) {
        self.renderer_state_machine.render();
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
    let font_atlas = Rc::new(load_font_atlas());
    let block_texture_atlas = create_block_texture_atlas();
    let background_panel_atlas = create_background_panel_atlas();
    let title_atlas = create_title_texture_atlas();
    let background_panel_height = height as usize;
    let background_panel_width = width as usize;
    let background_panel_spec = BackgroundPanelSpec { 
        height: background_panel_height, 
        width: background_panel_width, 
        background_atlas: &background_panel_atlas,
        title_atlas: &title_atlas,
    };
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
    let ui_gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 0_f32);
    let ui_trans_mat = Matrix4::one();
    let ui_panel_atlas = create_atlas_ui_panel();
    let ui_panel_spec = UIPanelSpec { 
        height: panel_height, 
        width: panel_width,
        atlas: &ui_panel_atlas,
    };
    let ui_panel_uniforms = UIPanelUniforms { gui_scale_mat: ui_gui_scale_mat, trans_mat: ui_trans_mat };
    let ui_panel = {
        let mut context = gl_context.borrow_mut();
        load_ui_panel(&mut *context, ui_panel_spec, ui_panel_uniforms)
    };
    
    let text_panel_uniforms = TextPanelUniforms { text_color: TEXT_COLOR };
    let text_panel_spec = TextPanelSpec {
        atlas: font_atlas.clone(),
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
    let next_block_cell = NextBlockCell::new();
    let next_piece = next_block_cell.block;
    let next_piece_panel_spec = NextPiecePanelSpec {
        piece: next_piece,
        atlas: &block_texture_atlas,
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
    let block_element_atlas = create_textures_playing_field(&block_texture_atlas);
    let playing_field_background_spec = PlayingFieldBackgroundSpec {
        width: 250,
        height: 500,
        atlas: &ui_panel_atlas,
    };
    let playing_field_background = {
        let mut context = gl_context.borrow_mut();
        load_playing_field_background(&mut context, playing_field_background_spec)
    };
    let playing_field_uniforms = create_uniforms_playing_field(488, viewport_width as u32, viewport_height as u32);
    let playing_field_spec = PlayingFieldHandleSpec {
        rows: 20,
        columns: 10,
        atlas: &block_element_atlas,
    };
    let playing_field_handle = {
        let mut context = gl_context.borrow_mut();
        load_playing_field(&mut *context, playing_field_spec, playing_field_uniforms)
    };
    let starting_block = GooglyBlock::new(GooglyBlockPiece::T, GooglyBlockRotation::R0);
    let starting_positions: HashMap<GooglyBlockPiece, BlockPosition> = [
        (GooglyBlockPiece::T, BlockPosition::new(-3, 4)),
        (GooglyBlockPiece::J, BlockPosition::new(-3, 4)), 
        (GooglyBlockPiece::Z, BlockPosition::new(-3, 4)),
        (GooglyBlockPiece::O, BlockPosition::new(-3, 4)), 
        (GooglyBlockPiece::S, BlockPosition::new(-3, 4)), 
        (GooglyBlockPiece::L, BlockPosition::new(-3, 4)),
        (GooglyBlockPiece::I, BlockPosition::new(-3, 3)),
    ].iter().map(|elem| *elem).collect();
    let playing_field_state_spec = PlayingFieldStateSpec {
        starting_block: starting_block,
        starting_positions: starting_positions,
    };
    let playing_field_state = Rc::new(RefCell::new(PlayingFieldState::new(playing_field_state_spec)));
    let playing_field = PlayingField::new(playing_field_handle, &block_element_atlas);
    let timer_spec = PlayingFieldTimerSpec {
        fall_interval: Interval::Milliseconds(500),
        collision_interval: Interval::Milliseconds(500),
        left_hold_interval: Interval::Milliseconds(70),
        right_hold_interval: Interval::Milliseconds(70),
        down_hold_interval: Interval::Milliseconds(35),
        rotate_interval: Interval::Milliseconds(100),
        clearing_interval: Interval::Milliseconds(60),
        flash_switch_interval: Interval::Milliseconds(50),
        flash_stop_interval: Interval::Milliseconds(500),
    };
    let next_block_cell_ref = Rc::new(RefCell::new(next_block_cell));
    let timers = Rc::new(RefCell::new(PlayingFieldTimers::new(timer_spec)));
    let statistics = Rc::new(RefCell::new(Statistics::new()));
    let score_board = Rc::new(RefCell::new(ScoreBoard::new()));
    let full_rows = Rc::new(RefCell::new(FullRows::new()));
    let game_over_panel_spec = GameOverPanelSpec {
        width: 300,
        height: 178,
        atlas: &ui_panel_atlas,
    };
    let game_over = {
        let mut context = gl_context.borrow_mut();
        load_game_over_panel(&mut context, game_over_panel_spec)
    };
    let flashing_state_machine = Rc::new(RefCell::new(FlashAnimationStateMachine::new()));
    let title_screen_state_machine_spec = TitleScreenStateMachineSpec {
        transition_interval: Interval::Milliseconds(2000),
        pressed_interval: Interval::Milliseconds(100),
        unpressed_interval: Interval::Milliseconds(500),
    };
    let title_screen = Rc::new(RefCell::new(TitleScreenStateMachine::new(title_screen_state_machine_spec)));
    let flashing_placement = AbsolutePlacement { x: 0.0, y: -0.7 };
    let title_screen_handle_spec = TitleScreenSpec {
        background_width: width as usize,
        background_height: height as usize,
        background_atlas: &title_atlas,
        flashing_width: 370,
        flashing_height: 50,
        flashing_placement: flashing_placement,
        flashing_atlas: &ui_panel_atlas,
    };
    let title_screen_handle = {
        let mut context = gl_context.borrow_mut();
        load_title_screen(&mut context, title_screen_handle_spec)
    };
    let context = Rc::new(RefCell::new(GameContext {
        gl: gl_context,
        timers: timers,
        playing_field_state: playing_field_state,
        statistics: statistics,
        score_board: score_board,
        next_block: next_block_cell_ref,
        full_rows: full_rows,
        flashing_state_machine: flashing_state_machine,
        exiting: false,
        title_screen: title_screen,
    }));
    let initial_game_state = GameState::TitleScreen(GameTitleScreenState::new());
    let state_machine = GameStateMachine::new(context.clone(), initial_game_state);
    let renderer_context = RendererContext {
        game_context: context.clone(),
        playing_field: playing_field,
        ui: ui,
        background: background,
        game_over: game_over,
        playing_field_background: playing_field_background,
        title_screen: title_screen_handle,
    };
    let initial_renderer_state = RendererState::TitleScreen(RendererTitleScreenState {});
    let renderer_state_machine = RendererStateMachine::new(renderer_context, initial_renderer_state); 

    let mut game = Game {
        context: context,
        state_machine: state_machine,
        renderer_state_machine: renderer_state_machine,
    };
    game.init_gpu();

    game
}

fn main() {
    let mut game = init_game();
    while !game.window_should_close() {
        let elapsed_milliseconds = game.update_timers();

        game.poll_events();
        match game.get_key(Key::Escape) {
            Action::Press => {
                let input = Input::new(InputKind::Exit, InputAction::Press);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Exit, InputAction::Press);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Left) {
            Action::Press => {
                let input = Input::new(InputKind::Left, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Left, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Right) {
            Action::Press => {
                let input = Input::new(InputKind::Right, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Right, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Down) {
            Action::Press => {
                let input = Input::new(InputKind::Down, InputAction::Press);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Down, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::R) {
            Action::Press => {
                let input = Input::new(InputKind::Rotate, InputAction::Press);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::Rotate, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }
        match game.get_key(Key::Enter) {
            Action::Press => {
                let input = Input::new(InputKind::StartGame, InputAction::Press);
                game.handle_input(input, elapsed_milliseconds);
            }
            Action::Repeat => {
                let input = Input::new(InputKind::StartGame, InputAction::Repeat);
                game.handle_input(input, elapsed_milliseconds);
            }
            _ => {}
        }

        game.update_state(elapsed_milliseconds);
        game.update_fps_counter();
        game.render();

        // Send the results to the output.
        game.swap_buffers();
    }

    info!("END LOG");
}
