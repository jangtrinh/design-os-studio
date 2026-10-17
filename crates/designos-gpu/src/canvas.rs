//! What an egui paint callback needs: the renderer plus the presenter.

use crate::exec::Renderer;
use crate::frame::Frame;
use crate::present::Presenter;

/// Stored in egui's callback resources: `prepare` runs a frame into the host
/// encoder, `paint` shows the result inside the host render pass.
pub struct Canvas {
    pub renderer: Renderer,
    presenter: Presenter,
}

impl Canvas {
    /// `target_format` / `samples` describe the host render pass the page is drawn into.
    pub fn new(dev: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat, samples: u32) -> Result<Self, crate::GpuError> {
        let renderer = Renderer::new(dev, queue)?;
        Ok(Self { renderer, presenter: Presenter::new(dev, target_format, samples) })
    }

    /// Renders the frame in a submission of its own, ahead of the host's.
    ///
    /// Frames whose uniforms span more than one ring chunk drew nothing when
    /// their passes were recorded into the host (egui) encoder: the 68,698-node
    /// `BW - DLS.fig` page at 3% zoom showed an empty canvas. Submitted on their
    /// own they render; queue order still puts them before the host pass that
    /// samples the output texture.
    pub fn prepare(&mut self, dev: &wgpu::Device, _enc: &mut wgpu::CommandEncoder, frame: &Frame) {
        let out = self.renderer.render_submitted(frame);
        self.presenter.set_source(dev, &out.view);
    }

    /// Draws the prepared frame into the host pass' viewport.
    pub fn paint(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.presenter.draw(pass);
    }

    pub fn max_texture_dimension(&self) -> u32 {
        self.renderer.max_texture_dimension()
    }
}
