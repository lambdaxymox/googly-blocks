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

// Green.
const TEXT_COLOR: [f32; 4] = [
    38_f32 / 255_f32, 239_f32 / 255_f32, 29_f32 / 255_f32, 255_f32 / 255_f32
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
    debug_assert!(tex > 0);

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
    let vert_source = include_shader!("background_panel.vert.glsl");
    let frag_source = include_shader!("background_panel.frag.glsl");

    ShaderSource { 
        vert_name: "background_panel.vert.glsl",
        vert_source: vert_source,
        frag_name: "background_panel.frag.glsl",
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
    debug_assert!(sp > 0);

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
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);
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
    debug_assert!(v_tex_vbo > 0);
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
    debug_assert!(vao > 0);
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
struct BackgroundPanelUniforms { 
    gui_scale_x: f32,
    gui_scale_y: f32,
}

fn send_to_gpu_uniforms_background_panel(sp: GLuint, uniforms: BackgroundPanelUniforms) {
    let gui_scale_mat = Matrix4::from_nonuniform_scale(
        uniforms.gui_scale_x, uniforms.gui_scale_y, 0.0
    );

    let m_gui_scale_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("m_gui_scale").as_ptr())
    };
    debug_assert!(m_gui_scale_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::UniformMatrix4fv(m_gui_scale_loc, 1, gl::FALSE, gui_scale_mat.as_ptr());
    }
}

#[derive(Copy, Clone)]
struct BackgroundPanelSpec { 
    height: usize, 
    width: usize, 
}

#[derive(Copy, Clone)]
struct BackgroundPanel {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
    height: usize,
    width: usize,
}

fn load_background(game: &mut glh::GLState, spec: BackgroundPanelSpec) -> BackgroundPanel {
    let shader_source = create_shaders_background();
    let mesh = create_geometry_background();
    let tex_image = create_textures_background();
    let sp = send_to_gpu_shaders_background(game, shader_source);
    let (v_pos_vbo, v_tex_vbo, vao) = send_to_gpu_geometry_background(sp, &mesh);
    let tex = send_to_gpu_textures_background(&tex_image);

    BackgroundPanel {
        sp: sp,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
        vao: vao,
        tex: tex,
        height: spec.height,
        width: spec.width,
    }
}

fn update_uniforms_background_panel(game: &mut Game) {
    let panel_width = game.background.width as f32;
    let panel_height = game.background.height as f32;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = BackgroundPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_background_panel(game.background.sp, uniforms);
}


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
    debug_assert!(sp > 0);

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
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe { gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr()) };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;

    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);
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
    debug_assert!(v_tex_vbo > 0);
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
    debug_assert!(vao > 0);
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

fn create_textures_ui_panel() -> TexImage2D {
    let arr: &'static [u8; 31235] = include_asset!("ui_panel.png");
    let asset = to_vec(&arr[0], 31235);

    teximage2d::load_from_memory(&asset).unwrap()
}

fn send_to_gpu_textures_ui_panel(tex_image: &TexImage2D) -> GLuint {
    send_to_gpu_texture(tex_image, gl::CLAMP_TO_EDGE).unwrap()
}

#[derive(Copy, Clone)]
struct UIPanelSpec {
    height: usize,
    width: usize,
}

struct UIPanel {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
    height: usize,
    width: usize,
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
    debug_assert!(ubo_index != gl::INVALID_INDEX);

    let mut ubo_size = 0;
    unsafe {
        gl::GetActiveUniformBlockiv(
            sp, ubo_index, gl::UNIFORM_BLOCK_DATA_SIZE, &mut ubo_size
        );
    }
    debug_assert!(ubo_size > 0);

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
    debug_assert!(ubo > 0);
    unsafe {
        gl::BindBuffer(gl::UNIFORM_BUFFER, ubo);
        gl::BufferData(
            gl::UNIFORM_BUFFER, ubo_size as GLsizeiptr,
            buffer.as_ptr() as *const GLvoid, gl::STATIC_DRAW
        );
        gl::BindBufferBase(gl::UNIFORM_BUFFER, ubo_index, ubo);
    }
}

fn load_ui_panel(game: &mut glh::GLState, spec: UIPanelSpec, uniforms: UIPanelUniforms) -> UIPanel {
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
        height: spec.height,
        width: spec.width,
    }
}

fn update_ui_panel_uniforms(game: &mut Game) {
    let panel_width = game.ui.ui_panel.width as f32;
    let panel_height = game.ui.ui_panel.height as f32;
    let (viewport_width, viewport_height) = game.get_framebuffer_size();
    let gui_scale_x = panel_width / (viewport_width as f32);
    let gui_scale_y = panel_height / (viewport_height as f32);
    let uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    send_to_gpu_uniforms_ui_panel(game.ui.ui_panel.sp, uniforms);    
}


#[derive(Copy, Clone, Debug)]
struct GLTextBuffer {
    sp: GLuint,
    v_pos_vbo: GLuint,
    v_tex_vbo: GLuint,
    vao: GLuint,
    tex: GLuint,
}

impl GLTextBuffer {
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

struct TextBuffer {
    points: Vec<f32>,
    tex_coords: Vec<f32>,
    gl_state: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    buffer: GLTextBuffer,
    scale_px: f32,
}

impl TextBuffer {
    fn new(
        gl_state: Rc<RefCell<glh::GLState>>, 
        atlas: Rc<BitmapFontAtlas>, 
        buffer: GLTextBuffer, scale_px: f32) -> TextBuffer {

        TextBuffer {
            points: vec![],
            tex_coords: vec![],
            gl_state: gl_state,
            atlas: atlas,
            buffer: buffer,
            scale_px: scale_px,
        }
    }

    fn clear(&mut self) {
        self.points.clear();
        self.tex_coords.clear();
    }

    fn write(&mut self, st: &[u8], placement: AbsolutePlacement) -> io::Result<(usize, usize)> {
        let atlas = &self.atlas;
        let scale_px = self.scale_px;
        let viewport_height: f32 = {
            let context = self.gl_state.borrow();
            context.height as f32
        };
        let viewport_width: f32 = {
            let context = self.gl_state.borrow();
            context.width as f32
        };

        // TODO: Optimize this.
        //let mut points = vec![0.0; 12 * st.len()];
        //let mut tex_coords = vec![0.0; 12 * st.len()];
        // END TODO.
        let mut at_x = placement.x;
        let at_y = placement.y;

        for (i, ch_i) in st.iter().enumerate() {
            let metadata_i = atlas.glyph_metadata[&(*ch_i as usize)];
            let atlas_col = metadata_i.column;
            let atlas_row = metadata_i.row;
            let atlas_rows = atlas.rows as f32;
            let atlas_columns = atlas.columns as f32;

            let s = (atlas_col as f32) * (1.0 / atlas_columns);
            let t = ((atlas_row + 1) as f32) * (1.0 / atlas_rows);

            let x_pos = at_x;
            let y_pos = at_y - (scale_px / viewport_height) * metadata_i.y_offset;

            at_x += metadata_i.width * (scale_px / viewport_width);


            self.points.push(x_pos);
            self.points.push(y_pos);
            self.points.push(x_pos);
            self.points.push(y_pos - scale_px / viewport_height);
            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos - scale_px / viewport_height);
            /*
            points[12 * i]     = x_pos;
            points[12 * i + 1] = y_pos;
            points[12 * i + 2] = x_pos;
            points[12 * i + 3] = y_pos - scale_px / (height as f32);
            points[12 * i + 4] = x_pos + scale_px / (width as f32);
            points[12 * i + 5] = y_pos - scale_px / (height as f32);
            */

            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos - scale_px / viewport_height);
            self.points.push(x_pos + scale_px / viewport_width);
            self.points.push(y_pos);
            self.points.push(x_pos);
            self.points.push(y_pos);
            /*
            points[12 * i + 6]  = x_pos + scale_px / (width as f32);
            points[12 * i + 7]  = y_pos - scale_px / (height as f32);
            points[12 * i + 8]  = x_pos + scale_px / (width as f32);
            points[12 * i + 9]  = y_pos;
            points[12 * i + 10] = x_pos;
            points[12 * i + 11] = y_pos;
            */
            
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t);
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t);            
            /*
            tex_coords[12 * i]     = s;
            tex_coords[12 * i + 1] = 1.0 - t + 1.0 / atlas_rows;
            tex_coords[12 * i + 2] = s;
            tex_coords[12 * i + 3] = 1.0 - t;
            tex_coords[12 * i + 4] = s + 1.0 / atlas_columns;
            tex_coords[12 * i + 5] = 1.0 - t;
            */
            
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t);
            self.tex_coords.push(s + 1.0 / atlas_columns);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
            self.tex_coords.push(s);
            self.tex_coords.push(1.0 - t + 1.0 / atlas_rows);
            /*
            tex_coords[12 * i + 6]  = s + 1.0 / atlas_columns;
            tex_coords[12 * i + 7]  = 1.0 - t;
            tex_coords[12 * i + 8]  = s + 1.0 / atlas_columns;
            tex_coords[12 * i + 9]  = 1.0 - t + 1.0 / atlas_rows;
            tex_coords[12 * i + 10] = s;
            tex_coords[12 * i + 11] = 1.0 - t + 1.0 / atlas_rows;
            */
        }

        //self.points.append(&mut points);
        //self.tex_coords.append(&mut tex_coords);

        let point_count = 6 * st.len();

        Ok((st.len(), point_count))
    }

    fn send_to_gpu(&mut self) -> io::Result<(usize, usize)> {
        self.buffer.write(&self.points, &self.tex_coords)?;
        let points_written = self.points.len();
        let tex_coords_written = self.tex_coords.len();
        
        Ok((points_written, tex_coords_written))
    }
}


#[derive(Copy, Clone, Debug)]
struct TextPanelUniforms {
    text_color: [f32; 4],
}

struct TextPanelSpec {
    atlas: Rc<BitmapFontAtlas>,
    score_placement: AbsolutePlacement,
    lines_placement: AbsolutePlacement,
    level_placement: AbsolutePlacement,
    tetrises_placement: AbsolutePlacement,
    scale_px: f32,
}

#[derive(Copy, Clone, Debug)]
struct AbsolutePlacement {
    x: f32,
    y: f32,
}

struct TextElement7 {
    content: [u8; 7],
    placement: AbsolutePlacement,
}

struct TextElement4 {
    content: [u8; 4],
    placement: AbsolutePlacement,
}

struct TextPanel {
    buffer: TextBuffer,
    score: TextElement7,
    level: TextElement4,
    tetrises: TextElement4,
    lines: TextElement4,
}

impl TextPanel {
    fn update_panel(&mut self) {
        self.buffer.clear();
        self.buffer.write(&self.score.content, self.score.placement).unwrap();
        self.buffer.write(&self.level.content, self.level.placement).unwrap();
        self.buffer.write(&self.tetrises.content, self.tetrises.placement).unwrap();
        self.buffer.write(&self.lines.content, self.lines.placement).unwrap();
        self.buffer.send_to_gpu().unwrap();
    }

    fn update_score(&mut self, score: usize) {
        let d0 = score % 10;
        let d1 = ((score % 100) - d0) / 10;
        let d2 = ((score % 1000) - d1) / 100;
        let d3 = ((score % 10000) - d2) / 1000;
        let d4 = ((score % 100000) - d3) / 10000;
        let d5 = ((score % 1000000) - d4) / 100000;
        let d6 = ((score % 10000000) - d5) / 1000000;
        self.score.content[0] = d6 as u8 + 0x30;
        self.score.content[1] = d5 as u8 + 0x30;
        self.score.content[2] = d4 as u8 + 0x30;
        self.score.content[3] = d3 as u8 + 0x30;
        self.score.content[4] = d2 as u8 + 0x30;
        self.score.content[5] = d1 as u8 + 0x30;
        self.score.content[6] = d0 as u8 + 0x30;
    }

    fn update_level(&mut self, level: usize) {
        let d0 = level % 10;
        let d1 = ((level % 100) - d0) / 10;
        let d2 = ((level % 1000) - d1) / 100;
        let d3 = ((level % 10000) - d2) / 1000;
        self.level.content[0] = d3 as u8 + 0x30;
        self.level.content[1] = d2 as u8 + 0x30;
        self.level.content[2] = d1 as u8 + 0x30;
        self.level.content[3] = d0 as u8 + 0x30;
    }

    fn update_lines(&mut self, lines: usize) {
        let d0 = lines % 10;
        let d1 = ((lines % 100) - d0) / 10;
        let d2 = ((lines % 1000) - d1) / 100;
        let d3 = ((lines % 10000) - d2) / 1000;
        self.lines.content[0] = d3 as u8 + 0x30;
        self.lines.content[1] = d2 as u8 + 0x30;
        self.lines.content[2] = d1 as u8 + 0x30;
        self.lines.content[3] = d0 as u8 + 0x30;
    }

    fn update_tetrises(&mut self, tetrises: usize) {
        let d0 = tetrises % 10;
        let d1 = ((tetrises % 100) - d0) / 10;
        let d2 = ((tetrises % 1000) - d1) / 100;
        let d3 = ((tetrises % 10000) - d2) / 1000;
        self.tetrises.content[0] = d3 as u8 + 0x30;
        self.tetrises.content[1] = d2 as u8 + 0x30;
        self.tetrises.content[2] = d1 as u8 + 0x30;
        self.tetrises.content[3] = d0 as u8 + 0x30;
    }
}

/// Set up the geometry for rendering title screen text.
fn create_buffers_text_buffer(sp: GLuint) -> (GLuint, GLuint, GLuint) {
    let v_pos_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_pos").as_ptr())
    };
    debug_assert!(v_pos_loc > -1);
    let v_pos_loc = v_pos_loc as u32;

    let v_tex_loc = unsafe {
        gl::GetAttribLocation(sp, glh::gl_str("v_tex").as_ptr())
    };
    debug_assert!(v_tex_loc > -1);
    let v_tex_loc = v_tex_loc as u32;
    
    let mut v_pos_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_pos_vbo);
    }
    debug_assert!(v_pos_vbo > 0);

    let mut v_tex_vbo = 0;
    unsafe {
        gl::GenBuffers(1, &mut v_tex_vbo);
    }
    debug_assert!(v_tex_vbo > 0);

    let mut vao = 0;
    unsafe {
        gl::GenVertexArrays(1, &mut vao);
    }
    debug_assert!(vao > 0);
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

/// Load the shaders for a textbox buffer.
fn create_shaders_text_buffer() -> ShaderSource {
    let vert_source = include_shader!("text_panel.vert.glsl");
    let frag_source = include_shader!("text_panel.frag.glsl");

    ShaderSource { 
        vert_name: "text_panel.vert.glsl",
        vert_source: vert_source,
        frag_name: "text_panel.frag.glsl",
        frag_source: frag_source,
    }    
}

/// Send the shaders for a textbox buffer to the GPU.
fn send_to_gpu_shaders_text_buffer(game: &mut glh::GLState, source: ShaderSource) -> GLuint {
    send_to_gpu_shaders(game, source)
}

fn send_to_gpu_uniforms_text_buffer(sp: GLuint, uniforms: TextPanelUniforms) {
    let text_color_loc = unsafe {
        gl::GetUniformLocation(sp, glh::gl_str("text_color").as_ptr())
    };
    debug_assert!(text_color_loc > -1);
    unsafe {
        gl::UseProgram(sp);
        gl::Uniform4f(
            text_color_loc,
            uniforms.text_color[0], uniforms.text_color[1], 
            uniforms.text_color[2], uniforms.text_color[3]
        );
    }
}

fn create_text_buffer(
    gl_state: Rc<RefCell<glh::GLState>>, 
    atlas: Rc<BitmapFontAtlas>, scale_px: f32, uniforms: TextPanelUniforms) -> TextBuffer {
    
    let atlas_tex = send_to_gpu_font_texture(&atlas, gl::CLAMP_TO_EDGE).unwrap();
    let shader_source = create_shaders_text_buffer();
    let sp = {
        let mut context = gl_state.borrow_mut();
        send_to_gpu_shaders_text_buffer(&mut *context, shader_source)
    };
    let (v_pos_vbo, v_tex_vbo, vao) = create_buffers_text_buffer(sp);
    send_to_gpu_uniforms_text_buffer(sp, uniforms);

    let buffer = GLTextBuffer {
        sp: sp,
        tex: atlas_tex,
        vao: vao,
        v_pos_vbo: v_pos_vbo,
        v_tex_vbo: v_tex_vbo,
    };

    TextBuffer::new(gl_state, atlas, buffer, scale_px)
}

fn load_text_panel(gl_state: Rc<RefCell<glh::GLState>>, spec: &TextPanelSpec, uniforms: TextPanelUniforms) -> TextPanel {
    let buffer = create_text_buffer(gl_state, spec.atlas.clone(), spec.scale_px, uniforms);
    let score = TextElement7 { content: [0; 7], placement: spec.score_placement };
    let lines =  TextElement4 { content: [0; 4], placement: spec.lines_placement };
    let level =  TextElement4 { content: [0; 4], placement: spec.level_placement };
    let tetrises =  TextElement4 { content: [0; 4], placement: spec.tetrises_placement };

    TextPanel {
        buffer: buffer,
        score: score,
        level: level,
        tetrises: tetrises,
        lines: lines,
    }
}

/// Load texture image into the GPU.
fn send_to_gpu_font_texture(atlas: &BitmapFontAtlas, wrapping_mode: GLuint) -> Result<GLuint, String> {
    let mut tex = 0;
    unsafe {
        gl::GenTextures(1, &mut tex);
    }
    debug_assert!(tex > 0);

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

/// Load a file atlas.
fn load_font_atlas() -> bmfa::BitmapFontAtlas {
    let arr: &'static [u8; 192197] = include_asset!("NotoSans-Bold.bmfa");
    let contents = to_vec(&arr[0], 192197);
    let mut reader = io::Cursor::new(contents);
    let atlas = bmfa::from_reader(&mut reader).unwrap();

    atlas
}

struct UI {
    ui_panel: UIPanel,
    text_panel: TextPanel,
}

impl UI {
    fn update_panel(&mut self) {
        self.text_panel.update_panel();
    }

    fn update_score(&mut self, score: usize) {
        self.text_panel.update_score(score);
    }

    fn update_lines(&mut self, lines: usize) {
        self.text_panel.update_lines(lines);
    }

    fn update_level(&mut self, level: usize) {
        self.text_panel.update_level(level);
    }

    fn update_tetrises(&mut self, tetrises: usize) {
        self.text_panel.update_tetrises(tetrises);
    }
}

struct Game {
    gl: Rc<RefCell<glh::GLState>>,
    atlas: Rc<BitmapFontAtlas>,
    ui: UI,
    background: BackgroundPanel,
    score: usize,
    level: usize,
    lines: usize,
    tetrises: usize,
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
        update_uniforms_background_panel(self);
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
        self.ui.update_score(self.score);
        self.ui.update_lines(self.lines);
        self.ui.update_level(self.level);
        self.ui.update_tetrises(self.tetrises);
        self.ui.update_panel();
    }

    #[inline(always)]
    fn render_ui(&mut self) {
        unsafe {
            // Render the game board. We turn off depth testing to do so since this is
            // a 2D scene using 3D abstractions. Otherwise Z-Buffering would prevent us
            // from rendering the game board.
            gl::UseProgram(self.ui.ui_panel.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.ui_panel.tex);
            gl::BindVertexArray(self.ui.ui_panel.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::UseProgram(self.ui.text_panel.buffer.buffer.sp);
            gl::Disable(gl::DEPTH_TEST);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.ui.text_panel.buffer.buffer.tex);
            gl::BindVertexArray(self.ui.text_panel.buffer.buffer.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, 19 * 6);
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

fn init_game() -> Game {
    init_logger("googly-blocks.log");
    info!("BEGIN LOG");
    info!("build version: ??? ?? ???? ??:??:??");
    let width = 896;
    let height = 504;
    let gl_context = Rc::new(RefCell::new(init_gl(width, height)));
    let atlas = Rc::new(load_font_atlas());

    let background_panel_height = height as usize;
    let background_panel_width = width as usize;
    let background_panel_spec = BackgroundPanelSpec { height: background_panel_height, width: background_panel_width };
    let background = {
        let mut context = gl_context.borrow_mut(); 
        load_background(&mut *context, background_panel_spec)
    };
    let (viewport_width, viewport_height) = {
        let context = gl_context.borrow();
        context.window.get_framebuffer_size()
    };
    let viewport_width = viewport_width as f32;
    let viewport_height = viewport_height as f32;
    let panel_width = 642;
    let panel_height = 504;
    let gui_scale_x = (panel_width as f32) / viewport_width;
    let gui_scale_y = (panel_height as f32) / viewport_height;

    let ui_panel_spec = UIPanelSpec { height: panel_height, width: panel_width };
    let ui_panel_uniforms = UIPanelUniforms { gui_scale_x: gui_scale_x, gui_scale_y: gui_scale_y };
    let ui_panel = {
        let mut context = gl_context.borrow_mut();
        load_ui_panel(&mut *context, ui_panel_spec, ui_panel_uniforms)
    };
    
    let text_panel_uniforms = TextPanelUniforms { text_color: TEXT_COLOR };
    let text_panel_spec = TextPanelSpec {
        atlas: atlas.clone(),
        score_placement: AbsolutePlacement { x: 0.46, y: 0.11 },
        level_placement: AbsolutePlacement { x: 0.50, y: -0.21 },
        lines_placement: AbsolutePlacement { x: 0.50, y: -0.54 },
        tetrises_placement: AbsolutePlacement { x: 0.50, y: -0.87 },
        scale_px: 48.0,
    };
    let text_panel = load_text_panel(gl_context.clone(), &text_panel_spec, text_panel_uniforms);
    
    let ui = UI { 
        ui_panel: ui_panel,
        text_panel: text_panel,
    };

    Game {
        gl: gl_context,
        atlas: atlas,
        ui: ui,
        background: background,
        score: 0,
        level: 0,
        lines: 0,
        tetrises: 0,
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
