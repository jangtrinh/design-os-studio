//! Shader modules, bind group layouts and render pipelines.

use crate::frame::Instance;
use std::mem::size_of;

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const MSAA: u32 = 4;

/// Uniform structs shared with the shaders. Layouts are `repr(C)` and padded by hand.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Globals {
    pub origin: [f32; 2],
    pub size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClipParams {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub enabled: f32,
    pub pad: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CParams {
    pub rect: [f32; 4],
    pub src_origin: [f32; 2],
    pub src_size: [f32; 2],
    pub aux_origin: [f32; 2],
    pub aux_size: [f32; 2],
    pub snap_origin: [f32; 2],
    pub snap_size: [f32; 2],
    pub tint: [f32; 4],
    pub opacity: f32,
    pub has_tint: u32,
    pub aux_mode: u32,
    pub blend: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurParams {
    pub dir: [i32; 2],
    pub size: [i32; 2],
    pub radius: i32,
    pub clamp_edges: u32,
    pub sigma: f32,
    pub pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlitParams {
    pub src_extent: [f32; 2],
    pub dst_extent: [f32; 2],
    pub src_tex: [f32; 2],
    pub pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FxParams {
    pub origin: [i32; 2],
    pub size: [i32; 2],
    pub mask_origin: [i32; 2],
    pub mask_size: [i32; 2],
    pub grid: [i32; 2],
    pub seed: u32,
    pub has_mask: u32,
    pub amp: f32,
    pub cell: f32,
    pub ss: f32,
    pub depth: f32,
    pub light: [f32; 2],
    pub intensity: f32,
    pub refraction: f32,
    pub dispersion: f32,
    /// Viewport size of the pass (see `fx.wgsl`).
    pub out_w: f32,
    pub out_h: f32,
    pub pad: f32,
}

pub struct Layouts {
    pub fx: wgpu::BindGroupLayout,
    pub globals: wgpu::BindGroupLayout,
    pub clip: wgpu::BindGroupLayout,
    pub image: wgpu::BindGroupLayout,
    pub comp: wgpu::BindGroupLayout,
    pub blur: wgpu::BindGroupLayout,
    pub blit: wgpu::BindGroupLayout,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CompKind {
    Over,
    Erase,
    Blend,
    Replace,
}

pub struct Pipelines {
    pub draw_over: wgpu::RenderPipeline,
    pub draw_erase: wgpu::RenderPipeline,
    pub image_over: wgpu::RenderPipeline,
    pub image_erase: wgpu::RenderPipeline,
    /// Composite pipelines for multisampled targets, indexed by [`CompKind`].
    pub comp_ms: [wgpu::RenderPipeline; 4],
    /// Composite pipelines for single-sample targets.
    pub comp_1x: [wgpu::RenderPipeline; 4],
    pub blur: wgpu::RenderPipeline,
    pub blit: wgpu::RenderPipeline,
    pub displace: wgpu::RenderPipeline,
    pub glass: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn comp(&self, kind: CompKind, ms: bool) -> &wgpu::RenderPipeline {
        let set = if ms { &self.comp_ms } else { &self.comp_1x };
        &set[kind as usize]
    }
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages, size: usize) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(size as u64) },
        count: None,
    }
}

fn tex_entry(binding: u32, vis: wgpu::ShaderStages, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }
}

impl Layouts {
    pub fn new(dev: &wgpu::Device) -> Self {
        use wgpu::ShaderStages as S;
        let vf = S::VERTEX | S::FRAGMENT;
        let mk = |label: &str, entries: &[wgpu::BindGroupLayoutEntry]| {
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries })
        };
        Self {
            fx: mk(
                "fx",
                &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: S::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(size_of::<FxParams>() as u64),
                        },
                        count: None,
                    },
                    tex_entry(1, S::FRAGMENT, false),
                    tex_entry(2, S::FRAGMENT, false),
                    tex_entry(3, S::FRAGMENT, false),
                ],
            ),
            globals: mk(
                "globals",
                &[uniform_entry(0, vf, size_of::<Globals>()), tex_entry(1, S::FRAGMENT, true), sampler_entry(2, S::FRAGMENT), sampler_entry(3, S::FRAGMENT)],
            ),
            clip: mk("clip", &[uniform_entry(0, S::FRAGMENT, size_of::<ClipParams>()), tex_entry(1, S::FRAGMENT, false)]),
            image: mk("image", &[tex_entry(0, S::FRAGMENT, true)]),
            comp: mk(
                "comp",
                &[
                    uniform_entry(0, vf, size_of::<CParams>()),
                    tex_entry(1, S::FRAGMENT, false),
                    tex_entry(2, S::FRAGMENT, false),
                    tex_entry(3, S::FRAGMENT, false),
                ],
            ),
            blur: mk("blur", &[uniform_entry(0, S::FRAGMENT, size_of::<BlurParams>()), tex_entry(1, S::FRAGMENT, false)]),
            blit: mk("blit", &[uniform_entry(0, S::FRAGMENT, size_of::<BlitParams>()), tex_entry(1, S::FRAGMENT, true), sampler_entry(2, S::FRAGMENT)]),
        }
    }
}

fn over_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn erase_blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn instance_attrs() -> [wgpu::VertexAttribute; 7] {
    use wgpu::VertexFormat as F;
    [
        wgpu::VertexAttribute { format: F::Float32x4, offset: 0, shader_location: 1 },
        wgpu::VertexAttribute { format: F::Float32x4, offset: 16, shader_location: 2 },
        wgpu::VertexAttribute { format: F::Float32x4, offset: 32, shader_location: 3 },
        wgpu::VertexAttribute { format: F::Float32x4, offset: 48, shader_location: 4 },
        wgpu::VertexAttribute { format: F::Float32x4, offset: 64, shader_location: 5 },
        wgpu::VertexAttribute { format: F::Uint32x4, offset: 80, shader_location: 6 },
        wgpu::VertexAttribute { format: F::Sint32x4, offset: 96, shader_location: 7 },
    ]
}

fn composite_shader_source() -> String {
    let [r, g, b] = designos_render::LUMINANCE_MASK_COEFFS;
    let raw = include_str!("shaders/composite.wgsl");
    let expr = format!("dot(aux.rgb, vec3<f32>({r:.4}, {g:.4}, {b:.4}))");
    assert!(raw.contains(&expr), "composite.wgsl luminance coefficients must match designos_render::LUMINANCE_MASK_COEFFS");
    raw.replace("dot(aux.rgb, vec3<f32>(0.2126, 0.7152, 0.0722))", &expr)
}

impl Pipelines {
    pub fn new(dev: &wgpu::Device, l: &Layouts) -> Self {
        let draw_src = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("draw"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/draw.wgsl").into()),
        });
        let comp_wgsl = composite_shader_source();
        let comp_src = dev.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("composite"), source: wgpu::ShaderSource::Wgsl(comp_wgsl.into()) });
        let blur_src = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blur.wgsl").into()),
        });
        let fx_src = dev
            .create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("fx"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fx.wgsl").into()) });
        let blit_src = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
        });

        let layout = |label: &str, groups: &[&wgpu::BindGroupLayout]| {
            let g: Vec<Option<&wgpu::BindGroupLayout>> = groups.iter().map(|b| Some(*b)).collect();
            dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: &g, immediate_size: 0 })
        };
        let draw_layout = layout("draw", &[&l.globals, &l.clip]);
        let image_layout = layout("image", &[&l.globals, &l.clip, &l.image]);
        let comp_layout = layout("comp", &[&l.globals, &l.clip, &l.comp]);
        let blur_layout = layout("blur", &[&l.blur]);
        let blit_layout = layout("blit", &[&l.blit]);
        let fx_layout = layout("fx", &[&l.fx]);

        let mesh_attr = [wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 }];
        let inst_attr = instance_attrs();
        let vbufs = [
            wgpu::VertexBufferLayout { array_stride: 8, step_mode: wgpu::VertexStepMode::Vertex, attributes: &mesh_attr },
            wgpu::VertexBufferLayout { array_stride: size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &inst_attr },
        ];

        let ms = wgpu::MultisampleState { count: MSAA, mask: !0, alpha_to_coverage_enabled: false };
        let one = wgpu::MultisampleState::default();
        let tri = wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() };

        let draw = |label: &str, lay: &wgpu::PipelineLayout, fs: &str, blend: wgpu::BlendState| {
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(lay),
                vertex: wgpu::VertexState { module: &draw_src, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &vbufs },
                primitive: tri,
                depth_stencil: None,
                multisample: ms,
                fragment: Some(wgpu::FragmentState {
                    module: &draw_src,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let comp = |label: &str, fs: &str, blend: Option<wgpu::BlendState>, samples: bool| {
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&comp_layout),
                vertex: wgpu::VertexState { module: &comp_src, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
                primitive: tri,
                depth_stencil: None,
                multisample: if samples { ms } else { one },
                fragment: Some(wgpu::FragmentState {
                    module: &comp_src,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let comp_set = |samples: bool| {
            [
                comp("comp over", "fs_plain", Some(over_blend()), samples),
                comp("comp erase", "fs_plain", Some(erase_blend()), samples),
                comp("comp blend", "fs_blend", None, samples),
                comp("comp replace", "fs_replace", None, samples),
            ]
        };

        let full = |label: &str, lay: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, fs: &str| {
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(lay),
                vertex: wgpu::VertexState { module, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: tri,
                depth_stencil: None,
                multisample: one,
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            draw_over: draw("draw over", &draw_layout, "fs_main", over_blend()),
            draw_erase: draw("draw erase", &draw_layout, "fs_main", erase_blend()),
            image_over: draw("image over", &image_layout, "fs_image", over_blend()),
            image_erase: draw("image erase", &image_layout, "fs_image", erase_blend()),
            comp_ms: comp_set(true),
            comp_1x: comp_set(false),
            blur: full("blur", &blur_layout, &blur_src, "fs_blur"),
            blit: full("blit", &blit_layout, &blit_src, "fs_blit"),
            displace: full("displace", &fx_layout, &fx_src, "fs_displace"),
            glass: full("glass", &fx_layout, &fx_src, "fs_glass"),
        }
    }
}
