extern crate glfw;
extern crate bmfa;
extern crate cgmath;
extern crate mini_obj;
extern crate toml;
extern crate log;
extern crate cgcamera;
extern crate file_logger;
extern crate teximage2d;


mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

#[macro_use]
mod macros;

mod gl_help;

use cgcamera::{
    FrustumFov, CameraAttitude, PerspectiveFovCamera
};
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
/* ----------------------------------------------------------------------------------- */
/*
 * 
 *    LOAD BACKGROUND LAYER
 * 
 * 
 *
 * 
 * 
 * 
 * 
 *  
*/
/* ----------------------------------------------------------------------------------- */
#[derive(Copy, Clone)]
struct ShaderSource {
    vert_source: &'static str,
    frag_source: &'static str,
}

#[inline]
fn create_shaders_background() -> ShaderSource {
    let vert_source = include_shader!("background.vert.glsl");
    let frag_source = include_shader!("background.frag.glsl");

    ShaderSource { vert_source: vert_source, frag_source: frag_source }
}

fn send_to_gpu_shaders(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    let mut vert_reader = io::Cursor::new(source.vert_source);
    let mut frag_reader = io::Cursor::new(source.frag_source);
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, "background.vert.glsl",
        &mut frag_reader, "background.frag.glsl"
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
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
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
    let tex = send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

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
/* ----------------------------------------------------------------------------------- */
/*
 * 
 *    END LOAD BACKGROUND LAYER
 * 
 * 
 * 
 * 
 * 
 * 
 * 
*/
/* ----------------------------------------------------------------------------------- */

fn load_board_shaders(game: &mut glh::GLState) -> GLuint {
    let mut vert_reader = io::Cursor::new(include_shader!("board.vert.glsl"));
    let mut frag_reader = io::Cursor::new(include_shader!("board.frag.glsl"));
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, "board.vert.glsl",
        &mut frag_reader, "board.frag.glsl"
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn load_board_obj() -> ObjMesh {
    let points: Vec<[GLfloat; 3]> = vec![
        [1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [ 1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0], [-1.0,  1.0, 0.0], [-1.0, -1.0, 0.0]
    ];
    /*
    let points: Vec<[GLfloat; 3]> = vec![
        [0.516, 1.000, 0.000], [-0.516, -1.000, 0.000], [ 0.516, -1.000, 0.000],
        [0.516, 1.000, 0.000], [-0.516,  1.000, 0.000], [-0.516, -1.000, 0.000],
    ];
    */
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

fn load_board_mesh(sp: GLuint) -> (GLuint, GLuint, GLuint) {
    let mesh = load_board_obj();

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
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            mesh.tex_coords.len_bytes() as GLsizeiptr,
            mesh.tex_coords.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        )
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
        gl::VertexAttribPointer(v_pos_loc, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::VertexAttribPointer(v_tex_loc, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
        gl::EnableVertexAttribArray(v_pos_loc);
        gl::EnableVertexAttribArray(v_tex_loc);
    }

    (v_pos_vbo, v_tex_vbo, vao)
}

fn load_board_textures(game: &mut glh::GLState) -> GLuint {
    let arr: &'static [u8; 4826] = include_asset!("board.png");
    let asset = to_vec(&arr[0], 4826);
    let tex_image = teximage2d::load_from_memory(&asset).unwrap();
    let tex = send_to_gpu_texture(&tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

fn load_board_uniforms(game: &mut glh::GLState, sp: GLuint) {
    let model_mat = Matrix4::one();
    //let view_mat = game.camera.view_mat;
    //let proj_mat = game.camera.proj_mat;
    let view_mat = Matrix4::one();
    let proj_mat = Matrix4::one();

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

    let mut indices = [0; 3];
    let mut sizes = [0; 3];
    let mut offsets = [0; 3];
    let mut types = [0; 3];
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index,
            gl::UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES, indices.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 3, indices.as_ptr() as *const u32,
            gl::UNIFORM_OFFSET, offsets.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 3, indices.as_ptr() as *const u32,
            gl::UNIFORM_SIZE, sizes.as_mut_ptr()
        );
        gl::GetActiveUniformsiv(
            sp, 3, indices.as_ptr() as *const u32,
            gl::UNIFORM_TYPE, types.as_mut_ptr()
        );
    }

    // Copy the uniform block data into a buffer to be passed to the GPU.
    let mut buffer = vec![0 as u8; ubo_size as usize];
    unsafe {
        ptr::copy(&proj_mat, mem::transmute(&mut buffer[offsets[0] as usize]), 1);
        ptr::copy(&view_mat, mem::transmute(&mut buffer[offsets[1] as usize]), 1);
        ptr::copy(&model_mat, mem::transmute(&mut buffer[offsets[2] as usize]), 1);
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

struct Board {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

fn load_board(game: &mut glh::GLState) -> Board {
    let sp = load_board_shaders(game);
    let (v_pos_vbo, v_tex_vbo, vao) = load_board_mesh(sp);
    let tex = load_board_textures(game);
    load_board_uniforms(game, sp);

    Board {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
    }
}

/* ------------------------------------------------------------------------- */
/* ------------------------------- TEXT BOX RENDERING ---------------------- */
/* ------------------------------------------------------------------------- */
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
    pos_x: f32,
    pos_y: f32,
}

#[derive(Copy, Clone, Debug)]
struct RelativePlacement {
    offset_x: f32,
    offset_y: f32,
}

// TODO: Place the texture image handle into the textbox element data structure
// for when we actually render the text.
#[derive(Copy, Clone, Debug)]
struct TextBoxElement {
    sp: GLuint,
    tex: GLuint,
    vao: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    placement: RelativePlacement,
    scale_px: f32,
}

impl TextBoxElement {
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

#[derive(Clone, Debug)]
struct TextBox {
    name: String,
    placement: AbsolutePlacement,
    background: TextBoxBackground,
    label: TextBoxElement,
    content: TextBoxElement,
}

fn create_shaders_textbox_background(game: &mut glh::GLState) -> GLuint {
    let mut vert_reader = io::Cursor::new(include_shader!("textbox_background.vert.glsl"));
    let mut frag_reader = io::Cursor::new(include_shader!("textbox_background.frag.glsl"));
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, "textbox_background.vert.glsl",
        &mut frag_reader, "textbox_background.frag.glsl"
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn create_shaders_textbox_element(game: &mut glh::GLState) -> GLuint {
    let mut vert_reader = io::Cursor::new(include_shader!("textbox_element.vert.glsl"));
    let mut frag_reader = io::Cursor::new(include_shader!("textbox_element.frag.glsl"));
    let sp = glh::create_program_from_reader(
        game,
        &mut vert_reader, "textbox_element.vert.glsl",
        &mut frag_reader, "textbox_element.frag.glsl"
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn create_textbox_background_mesh() -> (ObjMesh, AbsolutePlacement) {
    let points: Vec<[GLfloat; 3]> = vec![
        [1.0, 1.0, 0.0], [-1.0,  1.0, 0.0], [-1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0], [-1.0, -1.0, 0.0], [ 1.0, -1.0, 0.0]        
    ];
    /*
    let points: Vec<[GLfloat; 3]> = vec![
        [0.4862, 0.2431, 0.0], [-0.4862,  0.2431, 0.0], [-0.4862, -0.2431, 0.0],
        [0.4862, 0.2431, 0.0], [-0.4862, -0.2431, 0.0], [ 0.4862, -0.2431, 0.0]
    ];
    */
    let tex_coords: Vec<[GLfloat; 2]> = vec![
        [1.0, 1.0], [0.0, 1.0], [0.0, 0.0],
        [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]
    ];
    let normals: Vec<[GLfloat; 3]> = vec![
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]
    ];
    let mesh = ObjMesh::new(points, tex_coords, normals);
    let top_left = AbsolutePlacement { pos_x: -0.4862,  pos_y: 0.2431 };

    (mesh, top_left)
}

fn send_to_gpu_textbox_background_mesh(sp: GLuint, placement: AbsolutePlacement) -> (GLuint, GLuint, GLuint) {
    let (mesh, top_left) = create_textbox_background_mesh();
    let mat_scale = Matrix4::one();
    let distance = cgmath::vec3((placement.pos_x - top_left.pos_x, placement.pos_y - top_left.pos_y, 0.0));
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
    /*
    let v_mat_trans_loc = unsafe { 
        gl::GetUniformLocation(sp, glh::gl_str("v_mat_trans").as_ptr())
    };
    assert!(v_mat_trans_loc > -1);
    */

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
        //gl::UniformMatrix4fv(v_mat_trans_loc, 1, gl::FALSE, mat_trans.as_ptr());
    }

    (v_pos_vbo, v_tex_vbo, vao)
}


fn send_to_gpu_textbox_background_texture(game: &mut glh::GLState) -> GLuint {
    let arr: &'static [u8; 934] = include_asset!("textbox_background.png");
    let asset = to_vec(&arr[0], 934);
    let tex_image = teximage2d::load_from_memory(&asset).unwrap();
    let tex = send_to_gpu_texture(&tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

/// Set up the geometry for rendering title screen text.
fn create_buffers_textbox_element(sp: GLuint) -> (GLuint, GLuint, GLuint) {
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

fn create_textbox_background(game: &mut glh::GLState, placement: AbsolutePlacement) -> TextBoxBackground {
    let background_sp = create_shaders_textbox_background(game);
    let (v_pos_vbo, v_tex_vbo, vao) = send_to_gpu_textbox_background_mesh(background_sp, placement);
    let background_tex = send_to_gpu_textbox_background_texture(game);
    
    TextBoxBackground {
        sp: background_sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: background_tex,
    }       
}

fn create_textbox_element(
    game: &mut glh::GLState, font_tex: GLuint, 
    offset_x: f32, offset_y: f32, scale_px: f32) -> TextBoxElement {
    
    let sp = create_shaders_textbox_element(game);
    let (v_pos_vbo, v_tex_vbo, vao) = create_buffers_textbox_element(sp);
    let placement = RelativePlacement { offset_x, offset_y };

    TextBoxElement {
        sp: sp,
        tex: font_tex,
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        placement: placement,
        scale_px: scale_px,
    }
}

fn create_textbox(
    game: &mut glh::GLState, 
    name: &str, font_tex: GLuint, pos_x: f32, pos_y: f32) -> TextBox {
    
    let name = String::from(name);
    let placement = AbsolutePlacement { pos_x, pos_y };
    let background = create_textbox_background(game, placement);
    let label = create_textbox_element(game, font_tex, 0.1, 0.1, 64.0);
    let content = create_textbox_element(game, font_tex, 0.1, 0.24, 64.0);

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

fn text_to_vbo(
    app: &mut Game, atlas: &BitmapFontAtlas, 
    placement: AbsolutePlacement, tb: &mut TextBoxElement, st: &str) -> io::Result<(usize, usize)> {
    
    let scale_px = tb.scale_px;
    let height = app.gl.height;
    let width = app.gl.width;

    let mut points = vec![0.0; 12 * st.len()];
    let mut texcoords = vec![0.0; 12 * st.len()];
    let mut at_x = placement.pos_x + tb.placement.offset_x;
    //let end_at_x = 0.95;
    let mut at_y = placement.pos_y - tb.placement.offset_y;

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
    tb.write(&points, &texcoords)?;

    Ok((st.len(), point_count))
}
/* ------------------------------------------------------------------------- */
/* --------------------------- END TEXT BOX RENDERING ---------------------- */
/* ------------------------------------------------------------------------- */

fn load_camera(width: f32, height: f32) -> PerspectiveFovCamera {
    let near = 0.1;
    let far = 100.0;
    let fov = 67.0;
    let aspect = width / height;
    let frustum = FrustumFov::new(near, far, fov, aspect);

    let cam_pos = math::vec3((0.0, 0.0, 1.0));
    let fwd = math::vec4((0.0, 0.0, 1.0, 0.0));
    let rgt = math::vec4((1.0, 0.0, 0.0, 0.0));
    let up  = math::vec4((0.0, 1.0, 0.0, 0.0));
    let axis = Vector3::new(0.0, 0.0, -1.0);
    let attitude = CameraAttitude::new(cam_pos, fwd, rgt, up, axis);

    PerspectiveFovCamera::new(frustum, attitude)
}

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
    game.gl.width = width;
    game.gl.height = height;

    let aspect = game.gl.width as f32 / game.gl.height as f32;
    let fov = game.camera.fov;
    let near = game.camera.near;
    let far = game.camera.far;
    game.camera.aspect = aspect;
    game.camera.proj_mat = math::perspective((fov, aspect, near, far));
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

struct Game {
    gl: glh::GLState,
    camera: PerspectiveFovCamera,
    atlas: BitmapFontAtlas,
    background: Background,
    board: Board,
    score_board: TextBox,
    level: usize,
    tetrises: usize,
    lines: usize,
    score: usize,
}

impl Game {
    #[inline]
    fn get_framebuffer_size(&self) -> (i32, i32) {
        self.gl.window.get_framebuffer_size()
    }
}

fn init_game() -> Game {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let width = 896;
    let height = 504;
    let mut gl_context = init_gl(width, height);
    let camera = load_camera(width as f32, height as f32);
    let atlas = load_font_atlas();
    let atlas_tex = send_to_gpu_font_texture(&atlas, gl::CLAMP_TO_EDGE).unwrap();
    let background = load_background(&mut gl_context);
    let board = load_board(&mut gl_context);
    let score_board = create_textbox(&mut gl_context, "SCORE", atlas_tex, 0.1, 0.1);
    let score = 0;
    let lines = 0;
    let tetrises = 0;
    let level = 0;

    Game {
        gl: gl_context,
        camera: camera,
        atlas: atlas,
        background: background,
        board: board,
        score_board: score_board,
        score: score,
        lines: lines,
        tetrises: tetrises,
        level: level,
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
        gl::Viewport(0, 0, game.gl.width as i32, game.gl.height as i32);
    }

    let atlas = load_font_atlas();

    while !game.gl.window.should_close() {
        // Check input.
        let elapsed_seconds = glh::update_timers(&mut game.gl);

        game.gl.glfw.poll_events();
        match game.gl.window.get_key(Key::Escape) {
            Action::Press | Action::Repeat => {
                game.gl.window.set_should_close(true);
            }
            _ => {}
        }

        // Update the game world.
        glh::update_fps_counter(&mut game.gl);

        let (width, height) = game.get_framebuffer_size();
        if (width != game.gl.width as i32) && (height != game.gl.height as i32) {
            glfw_framebuffer_size_callback(
                &mut game, width as u32, height as u32
            );
        }

        // Render the results.
        unsafe {
            // Clear the screen.
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gl::ClearColor(0.2, 0.2, 0.2, 1.0);
            gl::Viewport(0, 0, game.gl.width as i32, game.gl.height as i32);

            // Render the background.
            let background = &game.background;
            gl::UseProgram(background.sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background.tex);
            gl::BindVertexArray(background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            /*
            // Render the game board. We turn off depth testing to do so since this is
            // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
            // from rendering the game board.
            let board = &game.board;
            
            gl::UseProgram(board.sp);
            // TODO: Move this somewhere else.
            let gui_scale_loc = unsafe {
                gl::GetUniformLocation(board.sp, glh::gl_str("gui_scale").as_ptr())
            };
            assert!(gui_scale_loc > -1);
            
            let panel_width: f32 = 230.0;
            let panel_height: f32 = 442.0;
            let (viewport_width, viewport_height) = game.get_framebuffer_size();
            let x_scale = panel_width / (viewport_width as f32);
            let y_scale = panel_height / (viewport_height as f32);
            gl::Uniform2f(gui_scale_loc, x_scale, y_scale);

            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, board.tex);
            gl::BindVertexArray(board.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            // TODO: Render the blocks instanced.

            // TODO: Render the UI elements.

            /* ------------------------------------------------------------------ */
            /* ---------------------- BEGIN TEXT RENDERING ---------------------- */
            /* ------------------------------------------------------------------ */
            // TODO: Render the text.
            let tb =  game.score_board.clone();
            let placement = &tb.placement;
            let mut label = tb.label.clone();
            let mut content = tb.content.clone();
            text_to_vbo(&mut game, &atlas, *placement, &mut label, "SCORE").unwrap();
            text_to_vbo(&mut game, &atlas, *placement, &mut content, "0xDEADBEEF").unwrap();
            gl::UseProgram(tb.background.sp);

            /* SET THE GUI ELEMENT SCALE */
            // TODO: Move this somewhere else.
            let v_mat_gui_scale_loc = gl::GetUniformLocation(
                tb.background.sp, glh::gl_str("v_mat_gui_scale").as_ptr()
            );
            assert!(v_mat_gui_scale_loc > -1);
            
            let panel_width: f32 = 218.0;
            let panel_height: f32 = 109.0;
            let (viewport_width, viewport_height) = game.get_framebuffer_size();
            let x_scale = panel_width / (viewport_width as f32);
            let y_scale = panel_height / (viewport_height as f32);
            let gui_scale = Matrix4::from_nonuniform_scale(x_scale, y_scale, 0.0);
            gl::UniformMatrix4fv(v_mat_gui_scale_loc, 1, gl::FALSE, gui_scale.as_ptr());
            /* END SET THE GUI ELEMENT SCALE */

            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tb.background.tex);
            gl::BindVertexArray(tb.background.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(label.sp);
            /* SET THE TEXT COLOR */
            // TODO: Move this somewhere else.
            let text_color_loc = gl::GetUniformLocation(label.sp, glh::gl_str("text_color").as_ptr());
            assert!(text_color_loc > -1);
            gl::Uniform4fv(text_color_loc, 1, HEADING_COLOR.as_ptr());
            /* END SET THE TEXT COLOR. */
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, label.tex);
            gl::BindVertexArray(label.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 5);
            
            gl::UseProgram(content.sp);
            // TODO: Move this somewhere else.
            let text_color_loc = unsafe {
                gl::GetUniformLocation(content.sp, glh::gl_str("text_color").as_ptr())
            };
            assert!(text_color_loc > -1);
            gl::Uniform4fv(text_color_loc, 1, TEXT_COLOR.as_ptr());
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, content.tex);
            gl::BindVertexArray(content.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6 * 10);

            /* ------------------------------------------------------------------ */
            /* ----------------------- END TEXT RENDERING ----------------------- */
            /* ------------------------------------------------------------------ */
            // TODO: Render the googly eyes.
            */
        }

        // Send the results to the output.
        game.gl.window.swap_buffers();
    }

    info!("END LOG");
}
