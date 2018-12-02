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
    let game = init_game();
}
