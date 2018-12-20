extern crate glfw;
extern crate stb_image;
extern crate cgmath;
extern crate wavefront;
extern crate serde;
extern crate toml;
extern crate log;
extern crate file_logger;

#[macro_use]
extern crate serde_derive;

mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

mod camera;
mod gl_help;
mod mesh;
mod texture;

use camera::Camera;
use gl_help as glh;
use cgmath as math;

use glfw::{Action, Context, Key};
use gl::types::{GLfloat, GLint, GLuint, GLvoid, GLsizeiptr};
use log::{info};
use math::{Matrix4, Quaternion};
use mesh::ObjMesh;
use texture::TexImage2D;

use std::mem;
use std::process;
use std::path::{Path, PathBuf};
use std::ptr;


const SHADER_PATH: &str = "shaders";
const ASSET_PATH: &str = "assets";

// OpenGL extension constants.
const GL_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FE;
const GL_MAX_TEXTURE_MAX_ANISOTROPY_EXT: u32 = 0x84FF;


struct Game {
    gl: glh::GLState,
    camera: Camera,
}

fn asset_file<P: AsRef<Path>>(file: P) -> PathBuf {
    let asset_path = Path::new(ASSET_PATH);
    let path = asset_path.join(file);

    path
}

fn shader_file<P: AsRef<Path>>(file: P) -> PathBuf {
    let shader_path = Path::new(SHADER_PATH);
    let path = shader_path.join(file);

    path
}

///
/// Load texture image into the GPU.
///
fn load_texture(tex_data: &TexImage2D, wrapping_mode: GLuint) -> Result<GLuint, String> {
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

fn load_background_shaders(game: &mut Game) -> GLuint {
    let sp = glh::create_program_from_files(
        &game.gl,
        &shader_file("background.vert.glsl"),
        &shader_file("background.frag.glsl")
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn load_background_obj() -> ObjMesh {
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

fn load_background_mesh(game: &mut Game, sp: GLuint) -> (GLuint, GLuint, GLuint) {
    let mesh = load_background_obj();

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

fn load_background_textures(game: &mut Game) -> GLuint {
    let tex_image = texture::load_file(&asset_file("background.png")).unwrap();
    let tex = load_texture(&tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

struct Background {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

fn load_background(game: &mut Game) -> Background {
    let sp = load_background_shaders(game);
    let (v_pos_vbo, v_tex_vbo, vao) = load_background_mesh(game, sp);
    let tex = load_background_textures(game);

    Background {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
    }
}

fn load_board_shaders(game: &mut Game) -> GLuint {
    let sp = glh::create_program_from_files(
        &game.gl,
        &shader_file("board.vert.glsl"),
        &shader_file("board.frag.glsl")
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn load_board_obj() -> ObjMesh {
    let points: Vec<[GLfloat; 3]> = vec![
        [0.5, 1.0, 0.0], [-0.5, -1.0, 0.0], [ 0.5, -1.0, 0.0],
        [0.5, 1.0, 0.0], [-0.5,  1.0, 0.0], [-0.5, -1.0, 0.0],
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

fn load_board_mesh(game: &mut Game, sp: GLuint) -> (GLuint, GLuint, GLuint) {
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

fn load_board_textures(game: &mut Game) -> GLuint {
    let tex_image = texture::load_file(&asset_file("board.png")).unwrap();
    let tex = load_texture(&tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

fn load_board_uniforms(game: &mut Game, sp: GLuint) {
    let model_mat = Matrix4::one();
    let view_mat = game.camera.view_mat;
    //let proj_mat = game.camera.proj_mat;
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

fn load_board(game: &mut Game) -> Board {
    let sp = load_board_shaders(game);
    let (v_pos_vbo, v_tex_vbo, vao) = load_board_mesh(game, sp);
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

fn load_camera(width: f32, height: f32) -> Camera {
    let near = 0.1;
    let far = 100.0;
    let fov = 67.0;
    let aspect = width / height;

    let fwd = math::vec4((0.0, 0.0, 1.0, 0.0));
    let rgt = math::vec4((1.0, 0.0, 0.0, 0.0));
    let up  = math::vec4((0.0, 1.0, 0.0, 0.0));
    let cam_pos = math::vec3((0.0, 0.0, 1.0));

    let axis = Quaternion::new(0.0, 0.0, 0.0, -1.0);

    Camera::new(near, far, fov, aspect, cam_pos, fwd, rgt, up, axis)
}


///
/// The GLFW frame buffer size callback function. This is normally set using
/// the GLFW `glfwSetFramebufferSizeCallback` function, but instead we explicitly
/// handle window resizing in our state updates on the application side. Run this function
/// whenever the size of the viewport changes.
///
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

///
/// Initialize the logger.
///
fn init_logger(log_file: &str) {
    file_logger::init(log_file).expect("Failed to initialize logger.");
}

///
/// Create and OpenGL context.
///
fn init_gl(width: u32, height: u32) -> glh::GLState {
    let gl_state = match glh::start_gl(width, height) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Failed to Initialize OpenGL context. Got error:");
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    gl_state
}

fn init_game() -> Game {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let width = 720;
    let height = 480;
    let gl_state = init_gl(width, height);
    let camera = load_camera(width as f32, height as f32);

    Game {
        gl: gl_state,
        camera: camera,
    }
}

fn main() {
    let mut game = init_game();

    let background = load_background(&mut game);
    let board = load_board(&mut game);

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

        let (width, height) = game.gl.window.get_framebuffer_size();
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
            /*
            // Render the background.
            gl::UseProgram(background_sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background_tex);
            gl::BindVertexArray(background_vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
            */
            // Render the game board. We turn off depth testing to do so since this is
            // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
            // from rendering the game board.
            gl::UseProgram(board.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, board.tex);
            gl::BindVertexArray(board.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            // TODO: Render the blocks instanced.

            // TODO: Render the UI elements.

            // TODO: Render the text.
        }

        // Send the results to the output.
        game.gl.window.swap_buffers();
    }

    info!("END LOG");
}
