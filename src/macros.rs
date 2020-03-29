/*
 *  Googly Blocks is a video game.
 *  Copyright (C) 2018,2019,2029  Christopher Blanchard
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

macro_rules! concat_path {
    ($fragment:expr) => {
        concat!($fragment, "/")
    };
    ($fragment:expr, $($fragments:expr),+) => {
        concat!($fragment, "/", concat_path!($($fragments),+))
    }
}

macro_rules! asset_file {
    ($asset:expr) => {
        concat!(concat_path!("..", "assets"), $asset)
    }
}

macro_rules! include_asset {
    ($asset:expr) => {
        include_bytes!(asset_file!($asset))
    }
}

#[cfg(target_os = "mac_os")]
macro_rules! shader_file {
    ($shader:expr) => {
        concat!(concat_path!("..", "shaders", "330"), $shader)
    }
}

#[cfg(target_os = "windows")]
macro_rules! shader_file {
    ($shader:expr) => {
        concat!(concat_path!("..", "shaders", "460"), $shader)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
macro_rules! shader_file {
    ($shader:expr) => {
        concat!(concat_path!("..", "shaders", "460"), $shader)
    }
}

macro_rules! include_shader {
    ($shader:expr) => {
        include_str!(shader_file!($shader))
    }
}
