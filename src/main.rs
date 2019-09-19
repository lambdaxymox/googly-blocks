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
use math::{Array, One, Matrix4, Vector3};
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

const HEADING_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TEXT_COLOR: [f32; 4] = [
    0_f32 / 255_f32, 204_f32 / 255_f32, 0_f32 / 255_f32, 255_f32 / 255_f32
];


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
    assert!(tex > 0);

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

#[inline]
fn create_shaders_background() -> ShaderSource {
    let vert_source = include_shader!("background.vert.glsl");
    let frag_source = include_shader!("background.frag.glsl");

    ShaderSource { 
        vert_name: "background.vert.glsl",
        vert_source: vert_source,
        frag_name: "background.frag.glsl",
        frag_source: frag_source,
    }
}

fn send_to_gpu_shaders(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    let mut vert_reader = io::Cursor::new(source.vert_source);
    let mut frag_reader = io::Cursor::new(source.frag_source);
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, source.vert_name,
        &mut frag_reader, source.frag_name
    ).unwrap();
    assert!(sp > 0);

    sp
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

fn send_to_gpu_geometry_background(sp: GLuint, mesh: &ObjMesh) -> (GLuint, GLuint, GLuint) {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    assert!(v_pos_vbo > 0);
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
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    assert!(v_tex_vbo > 0);
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
    }

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    (v_pos_vbo, v_tex_vbo, vao)
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
struct Background {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

fn load_background(game: &mut glh::GLState) -> Background {
    let shader_source = create_shaders_background();
    let mesh = create_geometry_background();
    let tex_image = create_textures_background();
    let sp = send_to_gpu_shaders_background(game, shader_source);
    let (v_pos_vbo, v_tex_vbo, vao) = send_to_gpu_geometry_background(sp, &mesh);
    let tex = send_to_gpu_textures_background(&tex_image);

    Background {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
    }
}


#[inline]
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
    let mut vert_reader = io::Cursor::new(source.vert_source);
    let mut frag_reader = io::Cursor::new(source.frag_source);
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, source.vert_name,
        &mut frag_reader, source.frag_name,
    ).unwrap();
    assert!(sp > 0);

    sp
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

fn send_to_gpu_geometry_ui_panel(sp: GLuint, mesh: &ObjMesh) -> (GLuint, GLuint, GLuint) {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    assert!(v_pos_vbo > 0);
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
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    assert!(v_tex_vbo > 0);
    unsafe {
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
    }

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    (v_pos_vbo, v_tex_vbo, vao)
}
/*
fn create_textures_board() -> TexImage2D {
    let arr: &'static [u8; 4826] = include_asset!("board.png");
    let asset = to_vec(&arr[0], 4826);

    teximage2d::load_from_memory(&asset).unwrap()
}
*/
fn create_textures_ui_panel() -> TexImage2D {
    let arr: &'static [u8; 31235] = include_asset!("ui_panel.png");
    let asset = to_vec(&arr[0], 31235);

    teximage2d::load_from_memory(&asset).unwrap()
}

fn send_to_gpu_textures_ui_panel(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct BoardUniforms {
    gui_scale_x: f32,
    gui_scale_y: f32,
}
/*
fn send_to_gpu_uniforms_board(sp: GLuint, uniforms: BoardUniforms) {
    let trans_mat = Matrix4::one();
    let gui_scale_mat = Matrix4::from_nonuniform_scale(uniforms.gui_scale_x, uniforms.gui_scale_y, 0.0);

    let ubo_index = unsafe {
        gl::GetUniformBlockIndex(sp, glh::gl_str("Matrices").as_ptr())
    };
    assert!(ubo_index != gl::INVALID_INDEX);

    let mut ubo_size = 0;
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index, gl::UNIFORM_BLOCK_DATA_SIZE, &mut ubo_size
        );
    }
    assert!(ubo_size > 0);

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
    assert!(ubo > 0);
    unsafe {
        gl::BindBuffer(gl::UNIFORM_BUFFER, ubo);
        gl::BufferData(
            gl::UNIFORM_BUFFER, ubo_size as GLsizeiptr,
            buffer.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindBufferBase(gl::UNIFORM_BUFFER, ubo_index, ubo);
    }
}
*/
/*
struct Board {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

fn load_board(game: &mut glh::GLState, uniforms: BoardUniforms) -> Board {
    let shader_source = create_shaders_board();
    let sp = send_to_gpu_shaders_board(game, shader_source);
    let mesh = create_geometry_board();
    let (v_pos_vbo, v_tex_vbo, vao) = send_to_gpu_geometry_board(sp, &mesh);
    let tex_image = create_textures_board();
    let tex = send_to_gpu_textures_board(&tex_image);
    send_to_gpu_uniforms_board(sp, uniforms);

    Board {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
    }
}

fn update_board_uniforms(game: &mut Game) {
    let panel_width: f32 = 642.0;
    let panel_height: f32 = 504.0;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = BoardUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_board(game.ui.board.sp, uniforms);
}
*/

struct UIPanel {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
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
    assert!(ubo_index != gl::INVALID_INDEX);

    let mut ubo_size = 0;
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index, gl::UNIFORM_BLOCK_DATA_SIZE, &mut ubo_size
        );
    }
    assert!(ubo_size > 0);

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
    assert!(ubo > 0);
    unsafe {
        gl::BindBuffer(gl::UNIFORM_BUFFER, ubo);
        gl::BufferData(
            gl::UNIFORM_BUFFER, ubo_size as GLsizeiptr,
            buffer.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindBufferBase(gl::UNIFORM_BUFFER, ubo_index, ubo);
    }
}

fn load_ui_panel(game: &mut glh::GLState, uniforms: UIPanelUniforms) -> UIPanel {
    let shader_source = create_shaders_ui_panel();
    let sp = send_to_gpu_shaders_ui_panel(game, shader_source);
    let mesh = create_geometry_ui_panel();
    let (v_pos_vbo, v_tex_vbo, vao) = send_to_gpu_geometry_ui_panel(sp, &mesh);
    let tex_image = create_textures_ui_panel();
    let tex = send_to_gpu_textures_ui_panel(&tex_image);
    send_to_gpu_uniforms_ui_panel(sp, uniforms);

    UIPanel {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
    }
}

fn update_ui_panel_uniforms(game: &mut Game) {
    let panel_width: f32 = 642.0;
    let panel_height: f32 = 504.0;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_ui_panel(game.ui.panel.sp, uniforms);    
}


#[derive(Copy, Clone, Debug)]
struct TextBoxBackground {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

#[derive(Copy, Clone, Debug)]
struct AbsolutePlacement {
    x: f32,
    y: f32,
}

#[derive(Copy, Clone, Debug)]
struct RelativePlacement {
    offset_x: f32,
    offset_y: f32,
}

#[derive(Clone)]
struct GLTextBoxBuffer {
    sp: GLuint,
    tex: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
}

impl GLTextBoxBuffer {
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


#[derive(Clone)]
struct TextBoxBuffer {
    buffer: GLTextBoxBuffer,
    gl_state: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    placement: RelativePlacement,
    scale_px: f32,
}

impl TextBoxBuffer {
    fn write(
        &mut self,
        placement: AbsolutePlacement, st: &str) -> io::Result<(usize, usize)> {
    
        let atlas = &self.atlas;
        let scale_px = self.scale_px;
        let height = {
            let context = self.gl_state.borrow();
            context.height
        };
        let width = {
            let context = self.gl_state.borrow();
            context.width
        };

        let mut points = vec![0.0; 12 * st.len()];
        let mut texcoords = vec![0.0; 12 * st.len()];
        let mut at_x = placement.x + self.placement.offset_x;
        //let end_at_x = 0.95;
        let mut at_y = placement.y - self.placement.offset_y;

        for (i, ch_i) in st.chars().enumerate() {
            let metadata_i = atlas.glyph_metadata[&(ch_i as usize)];
            let atlas_col = metadata_i.column;
            let atlas_row = metadata_i.row;

            let s = (atlas_col as f32) * (1.0 / (atlas.columns as f32));
            let t = ((atlas_row + 1) as f32) * (1.0 / (atlas.rows as f32));

            let x_pos = at_x;
            let y_pos = at_y - (scale_px / (height as f32)) * metadata_i.y_offset;

            at_x += metadata_i.width * (scale_px / width as f32);

            points[12 * i]     = x_pos;
            points[12 * i + 1] = y_pos;
            points[12 * i + 2] = x_pos;
            points[12 * i + 3] = y_pos - scale_px / (height as f32);
            points[12 * i + 4] = x_pos + scale_px / (width as f32);
            points[12 * i + 5] = y_pos - scale_px / (height as f32);

            points[12 * i + 6]  = x_pos + scale_px / (width as f32);
            points[12 * i + 7]  = y_pos - scale_px / (height as f32);
            points[12 * i + 8]  = x_pos + scale_px / (width as f32);
            points[12 * i + 9]  = y_pos;
            points[12 * i + 10] = x_pos;
            points[12 * i + 11] = y_pos;

            texcoords[12 * i]     = s;
            texcoords[12 * i + 1] = 1.0 - t + 1.0 / (atlas.rows as f32);
            texcoords[12 * i + 2] = s;
            texcoords[12 * i + 3] = 1.0 - t;
            texcoords[12 * i + 4] = s + 1.0 / (atlas.columns as f32);
            texcoords[12 * i + 5] = 1.0 - t;

            texcoords[12 * i + 6]  = s + 1.0 / (atlas.columns as f32);
            texcoords[12 * i + 7]  = 1.0 - t;
            texcoords[12 * i + 8]  = s + 1.0 / (atlas.columns as f32);
            texcoords[12 * i + 9]  = 1.0 - t + 1.0 / (atlas.rows as f32);
            texcoords[12 * i + 10] = s;
            texcoords[12 * i + 11] = 1.0 - t + 1.0 / (atlas.rows as f32);
        }

        let point_count = 6 * st.len();
        self.buffer.write(&points, &texcoords)?;

        Ok((st.len(), point_count))
    }
}

#[derive(Clone)]
struct TextBox {
    name: String,
    placement: AbsolutePlacement,
    background: TextBoxBackground,
    label: TextBoxBuffer,
    content: TextBoxBuffer,
}

fn create_shaders_textbox_background() -> ShaderSource {
    let vert_source = include_shader!("textbox_background.vert.glsl");
    let frag_source = include_shader!("textbox_background.frag.glsl");

    ShaderSource { 
        vert_name: "textbox_background.vert.glsl",
        vert_source: vert_source,
        frag_name: "textbox_background.frag.glsl",
        frag_source: frag_source,
    }
}

/// Send the background image for a textbox to the GPU.
fn send_to_gpu_shaders_textbox_background(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

/// Load the shaders for a textbox buffer.
fn create_shaders_textbox_buffer() -> ShaderSource {
    let vert_source = include_shader!("textbox_element.vert.glsl");
    let frag_source = include_shader!("textbox_element.frag.glsl");

    ShaderSource { 
        vert_name: "textbox_element.vert.glsl",
        vert_source: vert_source,
        frag_name: "textbox_element.frag.glsl",
        frag_source: frag_source,
    }    
}

/// Send the shaders for a textbox buffer to the GPU.
fn send_to_gpu_shaders_textbox_buffer(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

struct TextBoxGeometry {
    mesh: ObjMesh,
    top_left: AbsolutePlacement,
}

fn create_geometry_textbox_background() -> TextBoxGeometry {
    let points: Vec<[GLfloat; 3]> = vec![
        [1.0, 1.0, 0.0], [-1.0,  1.0, 0.0], [-1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [ 1.0, -1.0, 0.0]        
    ];
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]
    ];
    let normals: Vec<[GLfloat; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]
    ];
    let mesh = ObjMesh::new(points, tex_coords, normals);
    let top_left = AbsolutePlacement { x: -1.0,  y: 1.0 };

    TextBoxGeometry { mesh: mesh, top_left: top_left }
}

struct TextBoxGeometryHandle {
    v_pos_vbo: GLuint, 
    v_tex_vbo: GLuint,
    vao: GLuint,
}

/// Send the textbox background geometry to the GPU.
fn send_to_gpu_geometry_textbox_background(
    sp: GLuint, 
    placement: AbsolutePlacement, geometry: &TextBoxGeometry) -> TextBoxGeometryHandle {

    let mesh = &geometry.mesh;
    let top_left = geometry.top_left;
    let mat_scale = Matrix4::one();
    let distance = cgmath::vec3((placement.x - top_left.x, placement.y - top_left.y, 0.0));
    let mat_trans = Matrix4::from_translation(distance);

    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr())
    };
    assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let v_mat_scale_loc = unsafe { 
        gl::GetUniformLocation(sp, glh::gl_str("v_mat_gui_scale").as_ptr())
    };
    assert!(v_mat_scale_loc > -1);
    
    let v_mat_trans_loc = unsafe { 
        gl::GetUniformLocation(sp, glh::gl_str("v_mat_trans").as_ptr())
    };
    assert!(v_mat_trans_loc > -1);
    

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    assert!(v_pos_vbo > 0);
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
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    assert!(v_tex_vbo > 0);
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
    assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(v_mat_scale_loc, 1, gl::FALSE, mat_scale.as_ptr());
        gl::UniformMatrix4fv(v_mat_trans_loc, 1, gl::FALSE, mat_trans.as_ptr());
    }

    TextBoxGeometryHandle { v_pos_vbo, v_tex_vbo, vao }
}

/// Load the textbox background for the texture.
fn create_texture_textbox_background() -> TexImage2D {
    let arr: &'static [u8; 934] = include_asset!("textbox_background.png");
    let asset = to_vec(&arr[0], 934);

    teximage2d::load_from_memory(&asset).unwrap()
}

/// Send the background image texture of a textbox to the GPU.
fn send_to_gpu_texture_textbox_background(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

/// Set up the geometry for rendering title screen text.
fn create_buffers_textbox_buffer(sp: GLuint) -> (GLuint, GLuint, GLuint) {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr())
    };
    assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;
    
    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    assert!(vao > 0);
    unsafe {
        gl::BindVertexArray(vao);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_pos_vbo);
        gl::VertexAttribPointer(v_pos_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    (v_pos_vbo, v_tex_vbo, vao)
}

fn create_textbox_background(
    gl_state: &mut glh::GLState, 
    placement: AbsolutePlacement, panel_width: usize, panel_height: usize) -> TextBoxBackground {
    
    let shader_source = create_shaders_textbox_background();
    let sp = send_to_gpu_shaders_textbox_background(gl_state, shader_source);
    let mut geometry = create_geometry_textbox_background();

    let viewport_width = gl_state.width;
    let viewport_height = gl_state.height;
    let panel_scale_x = panel_width as f32 / viewport_width as f32;
    let panel_scale_y = panel_height as f32 / viewport_height as f32;
    let pos_x = panel_scale_x * geometry.top_left.x;
    let pos_y = panel_scale_y * geometry.top_left.y;
    geometry.top_left.x = pos_x;
    geometry.top_left.y = pos_y;

    let handle = send_to_gpu_geometry_textbox_background(sp, placement, &geometry);
    let tex_image = create_texture_textbox_background();
    let tex = send_to_gpu_texture_textbox_background(&tex_image);
    
    TextBoxBackground {
        sp: sp,
        v_pos_vbo: handle.v_pos_vbo,
        v_tex_vbo: handle.v_tex_vbo,
        vao: handle.vao,
        tex: tex,
    }
}

fn create_textbox_buffer(
    gl_state: Rc<RefCell<glh::GLState>>, 
    atlas: Rc<BitmapFontAtlas>, atlas_tex: GLuint, 
    offset_x: f32, offset_y: f32, scale_px: f32) -> TextBoxBuffer {
    
    let shader_source = create_shaders_textbox_buffer();
    let sp = {
        let mut context = gl_state.borrow_mut();
        send_to_gpu_shaders_textbox_buffer(&mut *context, shader_source)
    };
    let (v_pos_vbo, v_tex_vbo, vao) = create_buffers_textbox_buffer(sp);
    let placement = RelativePlacement { offset_x, offset_y };
    let buffer = GLTextBoxBuffer {
        sp: sp,
        tex: atlas_tex,
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
    };

    TextBoxBuffer {
        buffer: buffer,
        gl_state: gl_state,
        placement: placement,
        atlas: atlas,
        scale_px: scale_px,
    }
}

struct TextBoxSpec {
    name: &'static str,
    atlas: Rc<BitmapFontAtlas>,
    atlas_tex: GLuint,
    pos_x: f32,
    pos_y: f32,
    panel_width: usize,
    panel_height: usize,    
}

fn load_textbox(gl_state: Rc<RefCell<glh::GLState>>, spec: &TextBoxSpec) -> TextBox {
    let name = String::from(spec.name);
    let placement = AbsolutePlacement { x: spec.pos_x, y: spec.pos_y };
    let background = {
        let mut context = gl_state.borrow_mut();
        create_textbox_background(&mut *context, placement, spec.panel_width, spec.panel_height)
    };
    let label = create_textbox_buffer(
        gl_state.clone(), spec.atlas.clone(), spec.atlas_tex, 0.05, 0.1, 64.0
    );
    let content = create_textbox_buffer(
        gl_state.clone(), spec.atlas.clone(), spec.atlas_tex, 0.05, 0.24, 64.0
    );

    TextBox {
        name: name,
        placement: placement,
        background: background,
        label: label,
        content: content,
    }
}

/// Load texture image into the GPU.
fn send_to_gpu_font_texture(atlas: &BitmapFontAtlas, wrapping_mode: GLuint) -> Result<GLuint, String> {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
    }
    assert!(tex > 0);

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

fn update_panel_background(panel: &mut TextBox, viewport_width: u32, viewport_height: u32) {
    let v_mat_gui_scale_loc = unsafe { 
        gl::GetUniformLocation(panel.background.sp, glh::gl_str("v_mat_gui_scale").as_ptr())
    };
    assert!(v_mat_gui_scale_loc > -1);
            
    let panel_width: f32 = 218.0;
    let panel_height: f32 = 109.0;
    let x_scale = panel_width / (viewport_width as f32);
    let y_scale = panel_height / (viewport_height as f32);
    let gui_scale = Matrix4::from_nonuniform_scale(x_scale, y_scale, 0.0);
    unsafe {
        gl::UseProgram(panel.background.sp);
        gl::UniformMatrix4fv(v_mat_gui_scale_loc, 1, gl::FALSE, gui_scale.as_ptr());
    }    
}

fn update_panel_content(panel: &mut TextBox, label: &str, content: &str) {
    let placement = panel.placement;

    panel.label.write(placement, label).unwrap();
    panel.content.write(placement, content).unwrap();

    let text_color_loc = unsafe { 
        gl::GetUniformLocation(panel.label.buffer.sp, glh::gl_str("text_color").as_ptr())
    };
    assert!(text_color_loc > -1);
    let text_color_loc = unsafe {
        gl::GetUniformLocation(panel.content.buffer.sp, glh::gl_str("text_color").as_ptr())
    };
    assert!(text_color_loc > -1);

    unsafe {
        gl::UseProgram(panel.label.buffer.sp);
        gl::Uniform4fv(text_color_loc, 1, HEADING_COLOR.as_ptr());
        gl::UseProgram(panel.content.buffer.sp);
        gl::Uniform4fv(text_color_loc, 1, TEXT_COLOR.as_ptr());
    }
}

fn update_score_panel_background(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    update_panel_background(&mut game.ui.score_panel, viewport_width as u32, viewport_height as u32);
}

fn update_score_panel_content(game: &mut Game, content: &str) {
    let panel = &mut game.ui.score_panel;
    update_panel_content(panel, "SCORE", content);
}

fn update_level_panel_background(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    update_panel_background(&mut game.ui.level_panel, viewport_width as u32, viewport_height as u32);
}

fn update_level_panel_content(game: &mut Game, content: &str) {
    let panel = &mut game.ui.level_panel;
    update_panel_content(panel, "LEVEL", content);
}

fn update_line_panel_background(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    update_panel_background(&mut game.ui.line_panel, viewport_width as u32, viewport_height as u32);
}

fn update_line_panel_content(game: &mut Game, content: &str) {
    let panel = &mut game.ui.line_panel;
    update_panel_content(panel, "LINES", content);
}

fn update_tetris_panel_background(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    update_panel_background(&mut game.ui.tetris_panel, viewport_width as u32, viewport_height as u32);
}

fn update_tetris_panel_content(game: &mut Game, content: &str) {
    let panel = &mut game.ui.tetris_panel;
    update_panel_content(panel, "TETRISES", content);
}

fn update_next_panel_background(game: &mut Game) {
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    update_panel_background(&mut game.ui.next_panel, viewport_width as u32, viewport_height as u32);
}

fn update_next_panel_content(game: &mut Game, content: &str) {
    let panel = &mut game.ui.next_panel;
    update_panel_content(panel, "NEXT", content);
}

/// Load a file atlas.
fn load_font_atlas() -> bmfa::BitmapFontAtlas {
    let arr: &'static [u8; 115559] = include_asset!("googly_blocks.bmfa");
    let contents = to_vec(&arr[0], 115559);
    let mut reader = io::Cursor::new(contents);
    let atlas = bmfa::from_reader(&mut reader).unwrap();

    atlas
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

struct UI {
    panel: UIPanel,
    score_panel: TextBox,
    level_panel: TextBox,
    line_panel: TextBox,
    tetris_panel: TextBox,
    next_panel: TextBox,
}

struct Game {
    gl: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    ui: UI,
    background: Background,
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
    }

    #[inline(always)]
    fn render_background(&mut self) {
        unsafe {
            gl::UseProgram(self.background.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.background.tex);
            gl::BindVertexArray(self.background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }

    #[inline(always)]
    fn update_ui(&mut self) {
        update_ui_panel_uniforms(self);
        /*
        update_board_uniforms(self);
        update_score_panel_background(self);
        update_score_panel_content(self, "000000");
        update_level_panel_background(self);
        update_level_panel_content(self, "00");
        update_line_panel_background(self);
        update_line_panel_content(self, "000");
        update_tetris_panel_background(self);
        update_tetris_panel_content(self, "000");
        update_next_panel_background(self);
        update_next_panel_content(self, "DEADBEEF");
        */
    }

    #[inline(always)]
    fn render_ui(&mut self) {
        unsafe {
            // Render the game board. We turn off depth testing to do so since this is
            // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
            // from rendering the game board.
            gl::UseProgram(self.ui.panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.panel.tex);
            gl::BindVertexArray(self.ui.panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            /*
            let background = self.ui.score_panel.background;
            gl::UseProgram(background.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            let label = &self.ui.score_panel.label;
            gl::UseProgram(label.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.buffer.tex);
            gl::BindVertexArray(label.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 5);
            
            let content = &self.ui.score_panel.content;
            gl::UseProgram(content.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.buffer.tex);
            gl::BindVertexArray(content.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 8);

            let background = self.ui.level_panel.background;
            gl::UseProgram(background.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            let label = &self.ui.level_panel.label;
            gl::UseProgram(label.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.buffer.tex);
            gl::BindVertexArray(label.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 5);
            
            let content = &self.ui.level_panel.content;
            gl::UseProgram(content.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.buffer.tex);
            gl::BindVertexArray(content.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 8);

            let background = self.ui.line_panel.background;
            gl::UseProgram(background.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            let label = &self.ui.line_panel.label;
            gl::UseProgram(label.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.buffer.tex);
            gl::BindVertexArray(label.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 5);
            
            let content = &self.ui.line_panel.content;
            gl::UseProgram(content.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.buffer.tex);
            gl::BindVertexArray(content.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 4);

            let background = self.ui.tetris_panel.background;
            gl::UseProgram(background.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            let label = &self.ui.tetris_panel.label;
            gl::UseProgram(label.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.buffer.tex);
            gl::BindVertexArray(label.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 8);
            
            let content = &self.ui.tetris_panel.content;
            gl::UseProgram(content.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.buffer.tex);
            gl::BindVertexArray(content.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 3);
            
            let background = self.ui.next_panel.background;
            gl::UseProgram(background.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            
            let label = &self.ui.next_panel.label;
            gl::UseProgram(label.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.buffer.tex);
            gl::BindVertexArray(label.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 4);

            let content = &self.ui.next_panel.content;
            gl::UseProgram(content.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.buffer.tex);
            gl::BindVertexArray(content.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 8);
            */
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
    fn update_framebuffer_size(&mut self) {
        let (viewport_width, viewport_height) = self.get_framebuffer_size();
        let width = {
            let context = self.gl.borrow();
            context.width as i32
        };
        let height = {
            let context = self.gl.borrow();
            context.height as i32
        };
        if (width != viewport_width) && (height != viewport_height) {
            glfw_framebuffer_size_callback(
                self, viewport_width as u32, viewport_height as u32
            );
        }
    }
}

fn init_game() -> Game {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let width = 896;
    let height = 504;
    let gl_context = Rc::new(RefCell::new(init_gl(width, height)));
    let atlas = Rc::new(load_font_atlas());
    let atlas_tex = send_to_gpu_font_texture(&atlas, gl::CLAMP_TO_EDGE).unwrap();
    let background = {
        let mut context = gl_context.borrow_mut(); 
        load_background(&mut *context)
    };
    let (viewport_width, viewport_height) = {
        let context = gl_context.borrow();
        context.window.get_framebuffer_size()
    };
    let viewport_width = viewport_width as f32;
    let viewport_height = viewport_height as f32;
    let panel_width: f32 = 642.0;
    let panel_height: f32 = 504.0;
    let gui_scale_x = panel_width / viewport_width;
    let gui_scale_y = panel_height / viewport_height;
    let ui_panel_uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };

    let ui_panel = {
        let mut context = gl_context.borrow_mut();
        load_ui_panel(&mut *context, ui_panel_uniforms)
    };

    let score_panel_spec = TextBoxSpec {
        name: "SCORE",
        atlas: atlas.clone(),
        atlas_tex: atlas_tex,
        pos_x: 0.28,
        pos_y: 0.877,
        panel_width: 218,
        panel_height: 109,
    };
    let score_panel = load_textbox(gl_context.clone(), &score_panel_spec);

    let level_panel_spec = TextBoxSpec {
        name: "LEVEL",
        atlas: atlas.clone(),
        atlas_tex: atlas_tex,
        pos_x: -0.765,
        pos_y: 0.877,
        panel_width: 218,
        panel_height: 109,
    };
    let level_panel = load_textbox(gl_context.clone(), &level_panel_spec);

    let line_panel_spec = TextBoxSpec {
        name: "LINES",
        atlas: atlas.clone(),
        atlas_tex: atlas_tex,
        pos_x: -0.765,
        pos_y: 0.415,
        panel_width: 218,
        panel_height: 109,
    };
    let line_panel = load_textbox(gl_context.clone(), &line_panel_spec);

    let tetris_panel_spec = TextBoxSpec {
        name: "TETRISES",
        atlas: atlas.clone(),
        atlas_tex: atlas_tex,
        pos_x: -0.765,
        pos_y: -0.047,
        panel_width: 218,
        panel_height: 109,
    };
    let tetris_panel = load_textbox(gl_context.clone(), &tetris_panel_spec);

    let next_panel_spec = TextBoxSpec {
        name: "NEXT",
        atlas: atlas.clone(),
        atlas_tex: atlas_tex,
        pos_x: 0.28,
        pos_y: 0.415,
        panel_width: 218,
        panel_height: 109,
    };
    let next_panel = load_textbox(gl_context.clone(), &next_panel_spec);
    
    let ui = UI { 
        panel: ui_panel, 
        score_panel: score_panel,
        level_panel: level_panel,
        line_panel: line_panel,
        tetris_panel: tetris_panel,
        next_panel: next_panel,
    };

    Game {
        gl: gl_context,
        atlas: atlas,
        ui: ui,
        background: background,
    }
}

fn main() {
    let mut game = init_game();
    unsafe {
        // Enable depth testing.
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LESS);
        gl::Enable(gl::CULL_FACE);
        gl::CullFace(gl::BACK);
        gl::FrontFace(gl::CCW);
        // Gray background.
        gl::ClearColor(0.2, 0.2, 0.2, 1.0);
        let width = {
            let context = game.gl.borrow();
            context.width as i32
        };
        let height = {
            let context = game.gl.borrow();
            context.height as i32
        };
        gl::Viewport(0, 0, width, height);
    }

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

        // Render the results.
        unsafe {
            // Clear the screen.
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::ClearColor(0.2, 0.2, 0.2, 1.0);
            let width = {
                let context = game.gl.borrow();
                context.width as i32
            };
            let height = {
                let context = game.gl.borrow();
                context.height as i32
            };
            gl::Viewport(0, 0, width, height);

            // Render the background.
            game.update_background();
            game.render_background();

            // TODO: Render the UI completely.
            game.update_ui();
            game.render_ui();

            // TODO: Render the blocks instanced.

            // TODO: Render the googly eyes.
            
        }

        // Send the results to the output.
        game.swap_buffers();
    }

    info!("END LOG");
}
