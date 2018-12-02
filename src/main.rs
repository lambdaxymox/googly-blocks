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

mod gl_helpers;


use glfw::{Action, Context, Key};
use gl_helpers as glh;
use std::process;


struct GooglyBlocks {
    gl: glh::GLState,
}

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
    let gl_state = init_gl(720, 480);
    
    GooglyBlocks {
        gl: gl_state,
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

    while !game.gl.window.should_close() {
        match game.gl.window.get_key(Key::Escape) {
            Action::Press | Action::Repeat => {
                game.gl.window.set_should_close(true);
            }
            _ => {}
        }

        // Send the results to the output.
        game.gl.window.swap_buffers();
    }
}
