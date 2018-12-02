extern crate glfw;
extern crate log;

mod gl {
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}


mod gl_help;


pub use gl_help::*;
