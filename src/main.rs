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

mod gl_help;
mod texture;

use gl_help as glh;
use cgmath as math;

use glfw::{Action, Context, Key};
use gl::types::{GLfloat, GLint, GLuint, GLvoid, GLsizeiptr};
use log::{info};
use math::{Matrix4, AsArray};
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


struct GooglyBlocks {
    gl: glh::GLState,
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

fn init_game() -> GooglyBlocks {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let gl_state = init_gl(720, 480);
    
    GooglyBlocks {
        gl: gl_state,
    }
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

fn load_shader(game: &mut GooglyBlocks) -> GLuint {
    let sp = glh::create_program_from_files(
        &game.gl,
        &shader_file("background.vert.glsl"),
        &shader_file("background.frag.glsl")
    ).unwrap();
    assert!(sp > 0);

    sp
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

fn load_background_shaders(game: &mut GooglyBlocks) -> GLuint {
    let sp = glh::create_program_from_files(
        &game.gl,
        &shader_file("background.vert.glsl"),
        &shader_file("background.frag.glsl")
    ).unwrap();
    assert!(sp > 0);

    sp
}

fn load_background_mesh(game: &mut GooglyBlocks, sp: GLuint) -> (GLuint, GLuint, GLuint) {
    let mesh: [GLfloat; 18] = [
        1.0, 1.0, 0.0, -1.0, -1.0, 0.0,  1.0, -1.0, 0.0,
        1.0, 1.0, 0.0, -1.0,  1.0, 0.0, -1.0, -1.0, 0.0,
    ];
    let mesh_tex: [GLfloat; 12] = [
        1.0, 1.0, 0.0, 0.0, 1.0, 0.0,
        1.0, 1.0, 0.0, 1.0, 0.0, 0.0,
    ];

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
            (mem::size_of::<GLfloat>() * mesh.len()) as GLsizeiptr,
            mesh.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
    }

    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, v_tex_vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (mem::size_of::<GLfloat>() * mesh_tex.len()) as GLsizeiptr,
            mesh_tex.as_ptr() as *const GLvoid, gl::STATIC_DRAW
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

fn load_background_texture(game: &mut GooglyBlocks) -> GLuint {
    let tex_image = texture::load_file(&asset_file("background.png")).unwrap();
    let tex = load_texture(&tex_image, gl::CLAMP_TO_EDGE).unwrap();

    tex
}

fn load_uniforms2(game: &mut GooglyBlocks, sp: GLuint) -> (GLint, GLint, GLint) {
    let model_mat = Matrix4::one();
    let view_mat = Matrix4::one();
    let proj_mat = Matrix4::one();

    let sp_model_mat_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("model_mat").as_ptr())
    };
    assert!(sp_model_mat_loc > -1);

    let sp_view_mat_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("view_mat").as_ptr())
    };
    assert!(sp_view_mat_loc > -1);

    let sp_proj_mat_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("proj_mat").as_ptr())
    };
    assert!(sp_proj_mat_loc > -1);

    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(
            sp_model_mat_loc, 1, gl::FALSE,
            model_mat.as_ptr()
        );
        gl::UniformMatrix4fv(
            sp_view_mat_loc, 1, gl::FALSE,
            view_mat.as_ptr()
        );
        gl::UniformMatrix4fv(
            sp_proj_mat_loc, 1, gl::FALSE,
            proj_mat.as_ptr()
        );
    }

    (sp_model_mat_loc, sp_view_mat_loc, sp_proj_mat_loc)
}

///
/// The GLFW frame buffer size callback function. This is normally set using
/// the GLFW `glfwSetFramebufferSizeCallback` function, but instead we explicitly
/// handle window resizing in our state updates on the application side. Run this function
/// whenever the size of the viewport changes.
///
#[inline]
fn glfw_framebuffer_size_callback(game: &mut GooglyBlocks, width: u32, height: u32) {
    game.gl.width = width;
    game.gl.height = height;
}

fn main() {
    let mut game = init_game();

    let sp = load_background_shaders(&mut game);
    let (v_pos_vbo, v_tex_vbo, vao) = load_background_mesh(&mut game, sp);
    let background_tex = load_background_texture(&mut game);

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

            // Render the background.
            gl::UseProgram(sp);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, background_tex);
            gl::BindVertexArray(vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            // TODO: Render the game board.

            // TODO: Render the blocks instanced.

            // TODO: Render the UI elements.

            // TODO: Render the text.
        }

        // Send the results to the output.
        game.gl.window.swap_buffers();
    }

    info!("END LOG");
}
