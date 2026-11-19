use designos_gpu::{GpuContext, Renderer};

#[test]
fn probe_adapter() {
    match GpuContext::new() {
        Ok(c) => {
            eprintln!("adapter: {:?}", c.adapter_info);
            Renderer::new(&c.device, &c.queue).expect("renderer pipelines must validate");
        }
        Err(e) => eprintln!("SKIP: {e}"),
    }
}
