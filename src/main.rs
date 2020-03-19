extern crate glfw;
extern crate bmfa;
extern crate cgmath;
extern crate mini_obj;
extern crate toml;
extern crate log;
extern crate file_logger;
extern crate teximage2d;


mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[macro_use]
mod macros;

mod gl_help;


use gl_help as glh;
use cgmath as math;
use mini_obj as mesh;

use bmfa::BitmapFontAtlas;
use glfw::{Action, Context, Key};
use gl::types::{GLfloat, GLint, GLuint, GLvoid, GLsizeiptr};
use log::{info};
use math::{Array, One, Matrix4};
use mesh::ObjMesh;
use teximage2d::TexImage2D;

use std::io;
use std::mem;
use std::ptr;
use std::rc::Rc;
use std::cell::RefCell;

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

fn to_vec(ptr: *const u8, length: usize) -> Vec<u8> {
    let mut vec = vec![0 as u8; length];
    for i in 0..length {
        vec[i] = unsafe { *((ptr as usize + i) as *const u8) };
    }

    vec
}

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
    let points: Vec<[GLfloat; 3]> = vec![
        [1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [ 1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0], [-1.0,  1.0, 0.0], [-1.0, -1.0, 0.0],
    ];
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0],
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
    ];
    let normals: Vec<[GLfloat; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]
    ];

    ObjMesh::new(points, tex_coords, normals)
}

#[derive(Copy, Clone)]
struct BackgroundPanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
}

fn create_buffers_geometry_background(sp: GLuint) -> BackgroundPanelHandle {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

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
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    BackgroundPanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
    }    
}

fn send_to_gpu_geometry_background(sp: GLuint, handle: BackgroundPanelHandle, mesh: &ObjMesh) {
    let v_pos_loc = unsafe { 
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;
    
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
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }
}

fn create_textures_background() -> TexImage2D {
    let arr: &'static [u8; 27695] = include_asset!("title.png");
    let asset = to_vec(&arr[0], 27695);
    let tex_image = teximage2d::load_from_memory(&asset).unwrap();

    tex_image
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
    let handle = create_buffers_geometry_background(sp);
    send_to_gpu_geometry_background(sp, handle, &mesh);
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
    let points: Vec<[GLfloat; 3]> = vec![
        [1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [ 1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0], [-1.0,  1.0, 0.0], [-1.0, -1.0, 0.0]
    ];
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0],
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
    ];
    let normals: Vec<[GLfloat; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]
    ];

    ObjMesh::new(points, tex_coords, normals)
}

#[derive(Copy, Clone)]
struct UIPanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
}

fn create_buffers_geometry_ui_panel(sp: GLuint, mesh: &ObjMesh) -> UIPanelHandle {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    /*
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
    }
    */
    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    /*
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
    }
    */

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    UIPanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
    }
}

fn send_to_gpu_geometry_ui_panel(sp: GLuint, handle: UIPanelHandle, mesh: &ObjMesh) {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;
    /*
    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);
    */
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
    }
    /*
    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);
    */
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
    }
    /*
    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    */
    unsafe {
        gl::BindVertexArray(handle.vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, handle.v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }
}

fn create_textures_ui_panel() -> TexImage2D {
    let arr: &'static [u8; 31235] = include_asset!("ui_panel.png");
    let asset = to_vec(&arr[0], 31235);

    teximage2d::load_from_memory(&asset).unwrap()
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
    let handle = create_buffers_geometry_ui_panel(sp, &mesh);
    send_to_gpu_geometry_ui_panel(sp, handle, &mesh);
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
    let points: Vec<[f32; 3]> = vec![
        [-0.5, 0.5, 0.0], [0.0, 1.0, 0.0], [-0.5, 1.0, 0.0],
        [-0.5, 0.5, 0.0], [0.0, 0.5, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 1.0, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 0.5, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 1.0, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 0.5, 0.0], [ 1.0, 1.0, 0.0],        
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
    ];

    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_j_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [-0.5, 0.5, 0.0], [0.0, 1.0, 0.0], [-0.5, 1.0, 0.0],
        [-0.5, 0.5, 0.0], [0.0, 0.5, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 1.0, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 0.5, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 1.0, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 0.5, 0.0], [ 1.0, 1.0, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.5, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.0, 0.0], [ 1.0, 0.5, 0.0],       
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],    
    ];
    
    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_z_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [-0.5, 0.5, 0.0], [0.0, 1.0, 0.0], [-0.5, 1.0, 0.0],
        [-0.5, 0.5, 0.0], [0.0, 0.5, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 1.0, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 0.5, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.5, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.0, 0.0], [ 1.0, 0.5, 0.0],
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
    ];    

    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_o_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [0.0, 0.5, 0.0], [0.5, 1.0, 0.0], [0.0, 1.0, 0.0],
        [0.0, 0.5, 0.0], [0.5, 0.5, 0.0], [0.5, 1.0, 0.0],
        [0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [0.0, 0.5, 0.0],
        [0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [0.5, 0.5, 0.0],
        [0.5, 0.5, 0.0], [1.0, 1.0, 0.0], [0.5, 1.0, 0.0],
        [0.5, 0.5, 0.0], [1.0, 0.5, 0.0], [1.0, 1.0, 0.0],
        [0.5, 0.0, 0.0], [1.0, 0.5, 0.0], [0.5, 0.5, 0.0],
        [0.5, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.5, 0.0],        
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
    ];

    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_s_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [-0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [-0.5, 0.5, 0.0],
        [-0.5, 0.0, 0.0], [0.0, 0.0, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 1.0, 0.0], [ 0.0, 1.0, 0.0],
        [ 0.0, 0.5, 0.0], [0.5, 0.5, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 1.0, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 0.5, 0.0], [ 1.0, 1.0, 0.0],        
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],    
    ];
    
    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_l_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [-0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [-0.5, 0.5, 0.0],
        [-0.5, 0.0, 0.0], [0.0, 0.0, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 1.0, 0.0], [ 0.5, 1.0, 0.0],
        [ 0.5, 0.5, 0.0], [1.0, 0.5, 0.0], [ 1.0, 1.0, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.5, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [1.0, 0.0, 0.0], [ 1.0, 0.5, 0.0],        
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
    ];

    ObjMesh::new(points, tex_coords, normals)
}

fn create_geometry_i_piece() -> ObjMesh {
    let points: Vec<[f32; 3]> = vec![
        [-1.0, 0.0, 0.0], [-0.5, 0.5, 0.0], [-1.0, 0.5, 0.0],
        [-1.0, 0.0, 0.0], [-0.5, 0.0, 0.0], [-0.5, 0.5, 0.0],
        [-0.5, 0.0, 0.0], [ 0.0, 0.5, 0.0], [-0.5, 0.5, 0.0],
        [-0.5, 0.0, 0.0], [ 0.0, 0.0, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [ 0.5, 0.5, 0.0], [ 0.0, 0.5, 0.0],
        [ 0.0, 0.0, 0.0], [ 0.5, 0.0, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [ 1.0, 0.5, 0.0], [ 0.5, 0.5, 0.0],
        [ 0.5, 0.0, 0.0], [ 1.0, 0.0, 0.0], [ 1.0, 0.5, 0.0],        
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
    let normals: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
    ];

    ObjMesh::new(points, tex_coords, normals)
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
    let arr: &'static [u8; 1448] = include_asset!("blocks.png");
    let asset = to_vec(&arr[0], 1448);
    let tex_image = teximage2d::load_from_memory(&asset).unwrap();

    tex_image
}

/// Send the shaders for a textbox buffer to the GPU.
fn send_to_gpu_shaders_next_piece_panel(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct NextPiecePanelHandle {
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
}

fn send_to_gpu_geometry_piece_mesh(sp: GLuint, mesh: &ObjMesh) -> NextPiecePanelHandle {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.points.len_bytes() as GLsizeiptr,
            mesh.points.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
    }

    let mut v_tex_vbo = 0;
    unsafe {
        gl::CreateBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
    }

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    NextPiecePanelHandle {
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
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

fn send_to_gpu_geometry_next_panel(sp: GLuint, meshes: &PieceMeshes) -> NextPanelHandles {
    let t_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.t);
    let j_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.j);
    let z_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.z);
    let o_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.o);
    let s_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.s);
    let l_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.l);
    let i_handle = send_to_gpu_geometry_piece_mesh(sp, &meshes.i);

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
    piece: TetrisPiece, scale: u32, viewport_width: u32, viewport_height: u32) -> PieceUniformsData {
    
    use TetrisPiece::*;

    let block_width = 2.0 * (scale as f32 / viewport_width as f32);
    let block_height = 2.0 * (scale as f32 / viewport_height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(block_width, block_height, 1.0);
    
    let trans_mat = match piece {
        T => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        J => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        Z => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        O => Matrix4::from_translation(cgmath::vec3((0.50, 0.43, 0.0))),
        S => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        L => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        I => Matrix4::from_translation(cgmath::vec3((0.555, 0.48, 0.0))),
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
    use TetrisPiece::*;

    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let scale = 50;
    let gui_scale_x = 2.0 * (scale as f32) / (viewport_width as f32);
    let gui_scale_y = 2.0 * (scale as f32) / (viewport_height as f32);
    let gui_scale_mat = Matrix4::from_nonuniform_scale(gui_scale_x, gui_scale_y, 1.0);
    let trans_mat = match game.next_piece {
        T => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        J => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        Z => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        O => Matrix4::from_translation(cgmath::vec3((0.50, 0.43, 0.0))),
        S => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        L => Matrix4::from_translation(cgmath::vec3((0.525, 0.43, 0.0))),
        I => Matrix4::from_translation(cgmath::vec3((0.555, 0.48, 0.0))),
    };
    let uniforms = PieceUniformsData { gui_scale_mat: gui_scale_mat, trans_mat: trans_mat };
    send_to_gpu_uniforms_next_piece_panel(game.ui.next_piece_panel.buffer.sp, &uniforms);
}

fn send_to_gpu_textures_next_piece_panel(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()  
}


#[derive(Copy, Clone, PartialEq, Eq)]
enum TetrisPiece { T, J, Z, O, S, L, I }

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
    fn handle(&self, piece: TetrisPiece) -> NextPiecePanelHandle {
        use TetrisPiece::*;
        match piece {
            T => self.t_handle, 
            J => self.j_handle,
            Z => self.z_handle,
            O => self.o_handle,
            S => self.s_handle,
            L => self.l_handle,
            I => self.i_handle,
        }
    }
}

fn create_next_piece_panel_buffer(gl_context: &mut glh::GLState, uniforms: &PieceUniformsData) -> GLNextPiecePanel {
    let shader_source = create_shaders_next_piece_panel();
    let sp = send_to_gpu_shaders_next_piece_panel(gl_context, shader_source);
    let tex_image = create_textures_next_piece_panel();
    let tex = send_to_gpu_textures_next_piece_panel(&tex_image);
    let meshes = create_geometry_next_piece_panel();
    let handles = send_to_gpu_geometry_next_panel(sp, &meshes);
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
    current_piece: TetrisPiece,
    buffer: GLNextPiecePanel,
}

impl NextPiecePanel {
    fn update(&mut self, piece: TetrisPiece) {
        self.current_piece = piece;
    }
}

struct NextPiecePanelSpec {
    piece: TetrisPiece,
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

struct TextElement4 {
    content: [u8; 4],
    placement: AbsolutePlacement,
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
        let d0 = score % 10;
        let d1 = ((score % 100) - d0) / 10;
        let d2 = ((score % 1000) - d1) / 100;
        let d3 = ((score % 10000) - d2) / 1000;
        let d4 = ((score % 100000) - d3) / 10000;
        let d5 = ((score % 1000000) - d4) / 100000;
        let d6 = ((score % 10000000) - d5) / 1000000;
        self.score.content[0] = d6 as u8 + 0x30;
        self.score.content[1] = d5 as u8 + 0x30;
        self.score.content[2] = d4 as u8 + 0x30;
        self.score.content[3] = d3 as u8 + 0x30;
        self.score.content[4] = d2 as u8 + 0x30;
        self.score.content[5] = d1 as u8 + 0x30;
        self.score.content[6] = d0 as u8 + 0x30;
    }

    fn update_level(&mut self, level: usize) {
        let d0 = level % 10;
        let d1 = ((level % 100) - d0) / 10;
        let d2 = ((level % 1000) - d1) / 100;
        let d3 = ((level % 10000) - d2) / 1000;
        self.level.content[0] = d3 as u8 + 0x30;
        self.level.content[1] = d2 as u8 + 0x30;
        self.level.content[2] = d1 as u8 + 0x30;
        self.level.content[3] = d0 as u8 + 0x30;
    }

    fn update_lines(&mut self, lines: usize) {
        let d0 = lines % 10;
        let d1 = ((lines % 100) - d0) / 10;
        let d2 = ((lines % 1000) - d1) / 100;
        let d3 = ((lines % 10000) - d2) / 1000;
        self.lines.content[0] = d3 as u8 + 0x30;
        self.lines.content[1] = d2 as u8 + 0x30;
        self.lines.content[2] = d1 as u8 + 0x30;
        self.lines.content[3] = d0 as u8 + 0x30;
    }

    fn update_tetrises(&mut self, tetrises: usize) {
        let d0 = tetrises % 10;
        let d1 = ((tetrises % 100) - d0) / 10;
        let d2 = ((tetrises % 1000) - d1) / 100;
        let d3 = ((tetrises % 10000) - d2) / 1000;
        self.tetrises.content[0] = d3 as u8 + 0x30;
        self.tetrises.content[1] = d2 as u8 + 0x30;
        self.tetrises.content[2] = d1 as u8 + 0x30;
        self.tetrises.content[3] = d0 as u8 + 0x30;
    }

    fn update_t_pieces(&mut self, t_pieces: usize) {
        let d0 = t_pieces % 10;
        let d1 = ((t_pieces % 100) - d0) / 10;
        let d2 = ((t_pieces % 1000) - d1) / 100;
        let d3 = ((t_pieces % 10000) - d2) / 1000;
        self.t_pieces.content[0] = d3 as u8 + 0x30;
        self.t_pieces.content[1] = d2 as u8 + 0x30;
        self.t_pieces.content[2] = d1 as u8 + 0x30;
        self.t_pieces.content[3] = d0 as u8 + 0x30;
    }

    fn update_j_pieces(&mut self, j_pieces: usize) {
        let d0 = j_pieces % 10;
        let d1 = ((j_pieces % 100) - d0) / 10;
        let d2 = ((j_pieces % 1000) - d1) / 100;
        let d3 = ((j_pieces % 10000) - d2) / 1000;
        self.j_pieces.content[0] = d3 as u8 + 0x30;
        self.j_pieces.content[1] = d2 as u8 + 0x30;
        self.j_pieces.content[2] = d1 as u8 + 0x30;
        self.j_pieces.content[3] = d0 as u8 + 0x30;        
    }
    
    fn update_z_pieces(&mut self, z_pieces: usize) {
        let d0 = z_pieces % 10;
        let d1 = ((z_pieces % 100) - d0) / 10;
        let d2 = ((z_pieces % 1000) - d1) / 100;
        let d3 = ((z_pieces % 10000) - d2) / 1000;
        self.z_pieces.content[0] = d3 as u8 + 0x30;
        self.z_pieces.content[1] = d2 as u8 + 0x30;
        self.z_pieces.content[2] = d1 as u8 + 0x30;
        self.z_pieces.content[3] = d0 as u8 + 0x30;  
    }

    fn update_o_pieces(&mut self, o_pieces: usize) {
        let d0 = o_pieces % 10;
        let d1 = ((o_pieces % 100) - d0) / 10;
        let d2 = ((o_pieces % 1000) - d1) / 100;
        let d3 = ((o_pieces % 10000) - d2) / 1000;
        self.o_pieces.content[0] = d3 as u8 + 0x30;
        self.o_pieces.content[1] = d2 as u8 + 0x30;
        self.o_pieces.content[2] = d1 as u8 + 0x30;
        self.o_pieces.content[3] = d0 as u8 + 0x30;          
    }

    fn update_s_pieces(&mut self, s_pieces: usize) {
        let d0 = s_pieces % 10;
        let d1 = ((s_pieces % 100) - d0) / 10;
        let d2 = ((s_pieces % 1000) - d1) / 100;
        let d3 = ((s_pieces % 10000) - d2) / 1000;
        self.s_pieces.content[0] = d3 as u8 + 0x30;
        self.s_pieces.content[1] = d2 as u8 + 0x30;
        self.s_pieces.content[2] = d1 as u8 + 0x30;
        self.s_pieces.content[3] = d0 as u8 + 0x30;          
    }

    fn update_l_pieces(&mut self, l_pieces: usize) {
        let d0 = l_pieces % 10;
        let d1 = ((l_pieces % 100) - d0) / 10;
        let d2 = ((l_pieces % 1000) - d1) / 100;
        let d3 = ((l_pieces % 10000) - d2) / 1000;
        self.l_pieces.content[0] = d3 as u8 + 0x30;
        self.l_pieces.content[1] = d2 as u8 + 0x30;
        self.l_pieces.content[2] = d1 as u8 + 0x30;
        self.l_pieces.content[3] = d0 as u8 + 0x30;          
    }

    fn update_i_pieces(&mut self, i_pieces: usize) {
        let d0 = i_pieces % 10;
        let d1 = ((i_pieces % 100) - d0) / 10;
        let d2 = ((i_pieces % 1000) - d1) / 100;
        let d3 = ((i_pieces % 10000) - d2) / 1000;
        self.i_pieces.content[0] = d3 as u8 + 0x30;
        self.i_pieces.content[1] = d2 as u8 + 0x30;
        self.i_pieces.content[2] = d1 as u8 + 0x30;
        self.i_pieces.content[3] = d0 as u8 + 0x30;          
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
fn create_buffers_text_buffer(sp: GLuint) -> TextBufferHandle {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr())
    };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;
    
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
    let handle = create_buffers_text_buffer(sp);
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
    let arr: &'static [u8; 192197] = include_asset!("NotoSans-Bold.bmfa");
    let contents = to_vec(&arr[0], 192197);
    let mut reader = io::Cursor::new(contents);
    let atlas = bmfa::from_reader(&mut reader).unwrap();

    atlas
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

    fn update_next_piece(&mut self, piece: TetrisPiece) {
        self.next_piece_panel.update(piece);
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

struct ViewportDimensions {
    width: i32,
    height: i32,
}

struct Game {
    gl: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    ui: UI,
    background: BackgroundPanel,
    score: usize,
    level: usize,
    lines: usize,
    tetrises: usize,
    statistics: Statistics,
    next_piece: TetrisPiece,
}

impl Game {
    #[inline(always)]
    fn get_framebuffer_size(&self) -> (i32, i32) {
        self.gl.borrow().window.get_framebuffer_size()
    }

    #[inline(always)]
    fn window_should_close(&self) -> bool {
        self.gl.borrow().window.should_close()
    }

    #[inline(always)]
    fn window_set_should_close(&mut self, close: bool) {
        self.gl.borrow_mut().window.set_should_close(close);
    }

    #[inline(always)]
    fn update_fps_counter(&mut self) {
        let mut context = self.gl.borrow_mut();
        glh::update_fps_counter(&mut *context);
    }

    #[inline(always)]
    fn update_timers(&mut self) -> f64 {
        let mut context = self.gl.borrow_mut();
        glh::update_timers(&mut *context)
    }

    #[inline(always)]
    fn swap_buffers(&mut self) {
        self.gl.borrow_mut().window.swap_buffers();
    }

    #[inline(always)]
    fn update_background(&mut self) {
        update_uniforms_background_panel(self);
    }

    #[inline(always)]
    fn render_background(&mut self) {
        unsafe {
            gl::UseProgram(self.background.buffer.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.background.buffer.tex);
            gl::BindVertexArray(self.background.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    #[inline(always)]
    fn update_ui(&mut self) {
        update_ui_panel_uniforms(self);
        update_uniforms_next_piece_panel(self);
        self.ui.update_score(self.score);
        self.ui.update_lines(self.lines);
        self.ui.update_level(self.level);
        self.ui.update_tetrises(self.tetrises);
        self.ui.update_statistics(&self.statistics);
        self.ui.update_next_piece(self.next_piece);
        self.ui.update_panel();
    }

    #[inline(always)]
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
            gl::BindVertexArray(self.ui.next_piece_panel.buffer.handle(self.next_piece).vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 3 * 8);
        }
    }

    #[inline(always)]
    fn poll_events(&mut self) {
        self.gl.borrow_mut().glfw.poll_events();
    }

    #[inline(always)]
    fn get_key(&self, key: Key) -> Action {
        self.gl.borrow().window.get_key(key)
    }

    #[inline(always)]
    fn viewport_dimensions(&self) -> ViewportDimensions {
        let (width, height) = {
            let context = self.gl.borrow();
            (context.width as i32, context.height as i32)
        };
        
        ViewportDimensions { width, height }
    }

    #[inline(always)]
    fn update_framebuffer_size(&mut self) {
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let dims = self.viewport_dimensions();
        if (dims.width != viewport_width) && (dims.height != viewport_height) {
            glfw_framebuffer_size_callback(
                self, viewport_width as u32, viewport_height as u32
            );
        }
    }
}

/// The GLFW frame buffer size callback function. This is normally set using
/// the GLFW `glfwSetFramebufferSizeCallback` function, but instead we explicitly
/// handle window resizing in our state updates on the application side. Run this function
/// whenever the size of the viewport changes.
#[inline]
fn glfw_framebuffer_size_callback(game: &mut Game, width: u32, height: u32) {
    let mut context = game.gl.borrow_mut();
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
    
    let next_piece = TetrisPiece::T;
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

    Game {
        gl: gl_context,
        atlas: atlas,
        ui: ui,
        background: background,
        score: 0,
        level: 0,
        lines: 0,
        tetrises: 0,
        statistics: Statistics {
            t_pieces: 0,
            j_pieces: 0,
            z_pieces: 0,
            o_pieces: 0,
            s_pieces: 0,
            l_pieces: 0,
            i_pieces: 0, 
        },
        next_piece: next_piece,
    }
}

fn main() {
    let mut game = init_game();
    unsafe {
        // Enable depth testing.
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LESS);
        // Clear the z-buffer and the frame buffer.
        gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
        gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);

        let dims = game.viewport_dimensions();
        gl::Viewport(0, 0, dims.width, dims.height);
    }
    let mut dt = 0.0;
    while !game.window_should_close() {
        // Check input.
        let elapsed_seconds = game.update_timers();

        game.poll_events();
        match game.get_key(Key::Escape) {
            Action::Press | Action::Repeat => {
                game.window_set_should_close(true);
            }
            _ => {}
        }

        // Update the game world.
        game.update_fps_counter();
        game.update_framebuffer_size();

        dt += elapsed_seconds;
        if dt >= 1.0 {
            use TetrisPiece::*;
            game.next_piece = match game.next_piece {
                T => J,
                J => Z,
                Z => O,
                O => S,
                S => L,
                L => I,
                I => T,
            };
            dt = 0.0;
        }

        // Render the results.
        unsafe {
            // Clear the screen and the depth buffer.
            gl::ClearBufferfv(gl::DEPTH, 0, &CLEAR_DEPTH[0] as *const GLfloat);
            gl::ClearBufferfv(gl::COLOR, 0, &CLEAR_COLOR[0] as *const GLfloat);
            
            let dims = game.viewport_dimensions();
            gl::Viewport(0, 0, dims.width, dims.height);

            // Render the background.
            game.update_background();
            game.render_background();

            // TODO: Render the UI completely.
            game.update_ui();
            game.render_ui();

            // TODO: Render the blocks.

            // TODO: Render the googly eyes.
            
        }

        // Send the results to the output.
        game.swap_buffers();
    }

    info!("END LOG");
}
