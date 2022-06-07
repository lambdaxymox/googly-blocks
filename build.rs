/**
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
extern crate gl_generator;

use gl_generator::{Registry, Api, Profile, Fallbacks, GlobalGenerator};
use std::env;
use std::fs::File;
use std::path::Path;


#[cfg(target_os = "macos")]
fn register_gl_api(file: &mut File) {
    Registry::new(Api::Gl, (3, 3), Profile::Core, Fallbacks::All, [])
        .write_bindings(GlobalGenerator, file)
        .unwrap();
}

#[cfg(target_os = "windows")]
fn register_gl_api(file: &mut File) {
    Registry::new(Api::Gl, (4, 6), Profile::Core, Fallbacks::All, [])
        .write_bindings(GlobalGenerator, file)
        .unwrap();
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn register_gl_api(file: &mut File) {
    Registry::new(Api::Gl, (4, 6), Profile::Core, Fallbacks::All, [])
        .write_bindings(GlobalGenerator, file)
        .unwrap();
}

fn main() {
    let dest = env::var("OUT_DIR").unwrap();
    let mut file = File::create(&Path::new(&dest).join("gl_bindings.rs")).unwrap();

    register_gl_api(&mut file);
}
