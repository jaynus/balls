use glow::HasContext;
use regex::Regex;
use rl_core::math::Mat4;
use std::path::Path;

const INCLUDE_DIRECTIVE: &str = "#include ((<[^>]+>)|\"([^\"]+)\")";

pub struct ShaderProgram {
    pub handle: <glow::Context as HasContext>::Program,
}

fn process_includes(include_path: &Path, src: &str) -> String {
    let mut final_glsl = String::from(src);

    let re = Regex::new(INCLUDE_DIRECTIVE).unwrap();
    for cap in re.captures_iter(&src) {
        let path = include_path.join(&cap[3]);

        let data = std::fs::read_to_string(path).unwrap();
        final_glsl = final_glsl.replace(&cap[0], &data)
    }
    final_glsl
}

impl ShaderProgram {
    pub fn compile_graphics<P>(
        gl: &glow::Context,
        include_dir: P,
        vertex_code: &str,
        fragment_code: &str,
    ) -> Result<Self, anyhow::Error>
    where
        P: AsRef<Path>,
    {
        let vertex_code = process_includes(include_dir.as_ref(), vertex_code);
        let fragment_code = process_includes(include_dir.as_ref(), fragment_code);

        // Process the vertex code with spriv-cross

        // 1. compile shaders from strings
        unsafe {
            // vertex shader
            let vertex = gl.create_shader(glow::VERTEX_SHADER).unwrap();
            gl.shader_source(vertex, &vertex_code);
            gl.compile_shader(vertex);
            if !gl.get_shader_compile_status(vertex) {
                return Err(anyhow::anyhow!(gl.get_shader_info_log(vertex)));
            }

            // fragment Shader
            let fragment = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
            gl.shader_source(fragment, &fragment_code);
            gl.compile_shader(fragment);
            if !gl.get_shader_compile_status(fragment) {
                return Err(anyhow::anyhow!(gl.get_shader_info_log(fragment)));
            }

            // shader Program
            let id = gl.create_program().unwrap();
            gl.attach_shader(id, vertex);
            gl.attach_shader(id, fragment);
            gl.link_program(id);
            if !gl.get_program_link_status(id) {
                return Err(anyhow::anyhow!(gl.get_program_info_log(id)));
            }

            gl.delete_shader(vertex);
            gl.delete_shader(fragment);

            Ok(Self { handle: id })
        }
    }

    pub fn bind(&self, gl: &glow::Context) {
        unsafe {
            gl.use_program(Some(self.handle));
        }
    }

    pub fn location(
        &self,
        gl: &glow::Context,
        name: &str,
    ) -> Option<<glow::Context as glow::HasContext>::UniformLocation> {
        unsafe { gl.get_uniform_location(self.handle, name) }
    }

    pub fn set_mat4(&self, gl: &glow::Context, name: &str, mat: &Mat4, transpose: bool) {
        unsafe {
            gl.uniform_matrix_4_f32_slice(
                self.location(gl, name).as_ref(),
                transpose,
                &mat.as_array(),
            );
        }
    }
}
