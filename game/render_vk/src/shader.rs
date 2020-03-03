use ash::{version::DeviceV1_0, vk};
use rl_core::failure;
use rl_render_pod::{
    shader::ShaderSource,
    shaderc::{IncludeType, ResolvedInclude},
    spirv_reflect::types::variable::ReflectShaderStageFlags,
};
use std::{fs::File, path::Path};

pub use rl_render_pod::shaderc::ShaderKind;

#[derive(Clone)]
pub struct Shader {
    source: ShaderSource,
    module: vk::ShaderModule,
    info: vk::ShaderModuleCreateInfo,
}

impl Shader {
    pub fn info(&self) -> &vk::ShaderModuleCreateInfo {
        &self.info
    }

    pub fn stage(&self) -> ReflectShaderStageFlags {
        self.source.stage()
    }

    pub fn entry_point(&self) -> &std::ffi::CString {
        &self.source.entry_point()
    }

    pub fn module(&self) -> vk::ShaderModule {
        self.module
    }

    pub fn from_src_path<P>(
        device: &ash::Device,
        kind: ShaderKind,
        path: P,
    ) -> Result<Self, failure::Error>
    where
        P: AsRef<Path>,
    {
        Self::from_src_reader(
            device,
            kind,
            path.as_ref().to_str().ok_or_else(|| {
                failure::err_msg("Failed to parse provided path for shader compiler")
            })?,
            File::open(path.as_ref())?,
        )
    }

    pub fn from_src_reader<R>(
        device: &ash::Device,
        kind: ShaderKind,
        name: &str,
        reader: R,
    ) -> Result<Self, failure::Error>
    where
        R: std::io::Read,
    {
        let source = ShaderSource::from_src_reader(kind, name, reader)?;

        let info = *vk::ShaderModuleCreateInfo::builder().code(source.as_slice_u32());

        Ok(Self {
            source,
            module: unsafe { device.create_shader_module(&info, None)? },
            info,
        })
    }
}

#[derive(Default, Clone)]
pub struct ShaderSet {
    pub vertex: Option<Shader>,
    pub fragment: Option<Shader>,
    pub compute: Option<Shader>,
    pub geometry: Option<Shader>,
    pub tess_control: Option<Shader>,
    pub tess_eval: Option<Shader>,
}

impl ShaderSet {
    pub fn from(shaders: &[Shader]) -> Result<Self, failure::Error> {
        let mut set = Self::default();

        for shader in shaders.iter() {
            let stage = shader.stage();

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
