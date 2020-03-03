use failure;
use spirv_reflect::types::variable::ReflectShaderStageFlags;
use std::{fs::File, path::Path};

pub use shaderc::{CompilationArtifact, IncludeType, ResolvedInclude, ShaderKind};

#[derive(Clone)]
pub struct ShaderSource {
    kind: ShaderKind,
    reflect: spirv_reflect::ShaderModule,
    entry_point: std::ffi::CString,
    source: String,
    spirv: Vec<u8>,
}

impl ShaderSource {
    pub fn entry_point(&self) -> &std::ffi::CString {
        &self.entry_point
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn as_slice_u8(&self) -> &[u8] {
        &self.spirv
    }

    pub fn stage(&self) -> ReflectShaderStageFlags {
        self.reflect.get_shader_stage()
    }

    #[allow(clippy::cast_ptr_alignment)]
    pub fn as_slice_u32(&self) -> &[u32] {
        unsafe {
            std::slice::from_raw_parts(self.spirv.as_ptr() as *const u32, self.spirv.len() / 4)
        }
    }

    pub fn from_src_path<P>(kind: ShaderKind, path: P) -> Result<Self, failure::Error>
    where
        P: AsRef<Path>,
    {
        Self::from_src_reader(
            kind,
            path.as_ref().to_str().ok_or_else(|| {
                failure::err_msg("Failed to parse provided path for shader compiler")
            })?,
            File::open(path.as_ref())?,
        )
    }

    pub fn from_src_reader<R>(
        kind: ShaderKind,
        name: &str,
        mut reader: R,
    ) -> Result<Self, failure::Error>
    where
        R: std::io::Read,
    {
        let mut source = String::with_capacity(4096);
        reader.read_to_string(&mut source)?;

        let mut compiler = shaderc::Compiler::new()
            .ok_or_else(|| failure::err_msg("Failed to create shader compiler"))?;
        let mut options = shaderc::CompileOptions::new()
            .ok_or_else(|| failure::err_msg("Failed to create shader compiler"))?;

        options.set_generate_debug_info();
        options.set_warnings_as_errors();
        options.set_optimization_level(shaderc::OptimizationLevel::Zero);
        options.set_include_callback(include_callback);
        let artifact =
            compiler.compile_into_spirv(source.as_str(), kind, name, "main", Some(&options))?;

        // Reflect it
        let reflect = spirv_reflect::create_shader_module(artifact.as_binary_u8())
            .map_err(failure::err_msg)?;

        Ok(Self {
            reflect,
            kind,
            source,
            spirv: artifact.as_binary_u8().to_vec(),
            entry_point: std::ffi::CString::new("main").unwrap(),
        })
    }
}

#[derive(Default, Clone)]
pub struct ShaderSourceSet {
    pub vertex: Option<ShaderSource>,
    pub fragment: Option<ShaderSource>,
    pub compute: Option<ShaderSource>,
    pub geometry: Option<ShaderSource>,
    pub tess_control: Option<ShaderSource>,
    pub tess_eval: Option<ShaderSource>,
}

impl ShaderSourceSet {
    pub fn from(shaders: &[ShaderSource]) -> Result<Self, failure::Error> {
        let mut set = Self::default();

        for shader in shaders.iter() {
            let stage = shader.reflect.get_shader_stage();

            if stage.contains(ReflectShaderStageFlags::VERTEX) {
                if set.vertex.is_none() {
                    set.vertex = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else if stage.contains(ReflectShaderStageFlags::FRAGMENT) {
                if set.fragment.is_none() {
                    set.fragment = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else if stage.contains(ReflectShaderStageFlags::COMPUTE) {
                if set.compute.is_none() {
                    set.compute = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else if stage.contains(ReflectShaderStageFlags::GEOMETRY) {
                if set.geometry.is_none() {
                    set.geometry = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else if stage.contains(ReflectShaderStageFlags::TESSELLATION_CONTROL) {
                if set.tess_control.is_none() {
                    set.tess_control = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else if stage.contains(ReflectShaderStageFlags::TESSELLATION_EVALUATION) {
                if set.tess_eval.is_none() {
                    set.tess_eval = Some(shader.clone());
                } else {
                    return Err(failure::err_msg("Duplicate shader stage in set"));
                }
            } else {
                unimplemented!("Unsupported shader stage")
            }
        }

        Ok(set)
    }
}

fn include_callback(
    path: &str,
    _kind: IncludeType,
    _caller: &str,
    _depth: usize,
) -> Result<ResolvedInclude, String> {
    Ok(ResolvedInclude {
        resolved_name: path.to_string(),
        content: std::fs::read_to_string(Path::new("assets/shaders/").join(path))
            .map_err(|e| e.to_string())?,
    })
}
