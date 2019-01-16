#![allow(dead_code)]
use stb_image::image;
use stb_image::image::LoadResult;
use std::path::Path;


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    #[inline]
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba { r, g, b, a }
    }
}

impl Default for Rgba {
    #[inline]
    fn default() -> Rgba {
        Rgba::new(0, 0, 0, 255)
    }
}

pub struct TexImage2D {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub data: Vec<Rgba>,
}

impl TexImage2D {
    pub fn new(width: u32, height: u32) -> TexImage2D {
        TexImage2D {
            width: width,
            height: height,
            depth: 4,
            data: vec![Rgba::default(); (width * height) as usize],
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        &self.data[0].r
    }
}

impl<'a> From<&'a image::Image<u8>> for TexImage2D {
    fn from(image: &'a image::Image<u8>) -> TexImage2D {
        let mut data = vec![];
        for chunk in image.data.chunks(4) {
            data.push(Rgba::new(chunk[0], chunk[1], chunk[2], chunk[3]));
        }

        TexImage2D {
            width: image.width as u32,
            height: image.height as u32,
            depth: image.depth as u32,
            data: data,
        }
    }
}


///
/// Load a PNG texture image from a reader or buffer.
///
pub fn load_from_memory(buffer: &[u8]) -> Result<TexImage2D, String> {
    let force_channels = 4;
    let mut image_data = match image::load_from_memory_with_depth(buffer, force_channels, false) {
        LoadResult::ImageU8(image_data) => image_data,
        LoadResult::Error(_) => {
            return Err(format!("ERROR: could not load image buffer."));
        }
        LoadResult::ImageF32(_) => {
            return Err(format!(
                "ERROR: Tried to load an image as byte vectors, got f32 image instead."
            ));
        }
    };

    let width = image_data.width;
    let height = image_data.height;

    // Check that the image size is a power of two.
    if (width & (width - 1)) != 0 || (height & (height - 1)) != 0 {
        eprintln!("WARNING: Texture buffer is not power-of-2 dimensions");
    }

    let width_in_bytes = 4 *width;
    let half_height = height / 2;
    for row in 0..half_height {
        for col in 0..width_in_bytes {
            let temp = image_data.data[row * width_in_bytes + col];
            image_data.data[row * width_in_bytes + col] = image_data.data[((height - row - 1) * width_in_bytes) + col];
            image_data.data[((height - row - 1) * width_in_bytes) + col] = temp;
        }
    }

    let tex_image = TexImage2D::from(&image_data);

    Ok(tex_image)
}


///
/// Load a PNG texture image from a file name.
///
pub fn load_file<P: AsRef<Path>>(file_path: P) -> Result<TexImage2D, String> {
    let force_channels = 4;
    let mut image_data = match image::load_with_depth(&file_path, force_channels, false) {
        LoadResult::ImageU8(image_data) => image_data,
        LoadResult::Error(_) => {
            let disp = file_path.as_ref().display();
            return Err(format!("ERROR: could not load {}", disp));
        }
        LoadResult::ImageF32(_) => {
            let disp = file_path.as_ref().display();
            return Err(
                format!("ERROR: Tried to load an image as byte vectors, got f32: {}", disp)
            );
        }
    };

    let width = image_data.width;
    let height = image_data.height;

    // Check that the image size is a power of two.
    if (width & (width - 1)) != 0 || (height & (height - 1)) != 0 {
        let disp = file_path.as_ref().display();
        eprintln!("WARNING: texture {} is not power-of-2 dimensions", disp);
    }

    let width_in_bytes = 4 * width;
    let half_height = height / 2;
    for row in 0..half_height {
        for col in 0..width_in_bytes {
            let temp = image_data.data[row * width_in_bytes + col];
            image_data.data[row * width_in_bytes + col] = image_data.data[((height - row - 1) * width_in_bytes) + col];
            image_data.data[((height - row - 1) * width_in_bytes) + col] = temp;
        }
    }

    let tex_image = TexImage2D::from(&image_data);

    Ok(tex_image)
}
