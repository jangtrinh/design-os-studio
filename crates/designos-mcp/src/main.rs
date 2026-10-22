use designos_mcp::server::DesignOSServer;
use rmcp::{transport::stdio, ServiceExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = match (args.next(), args.next(), args.next()) {
        (Some(flag), Some(path), None) if flag == "--workspace" => std::path::PathBuf::from(path),
        (Some(flag), None, None) if flag == "--list-lint-rules" => {
            for rule_id in [
                "overflow-parent",
                "overflowing-children",
                "clipped-partial",
                "clipped-full",
                "sibling-overlap",
                "overlapping-siblings",
                "text-truncated",
                "text-unexpected-wrap",
                "collapsed-size",
                "zero-bounds",
                "sizing-conflict",
                "text-missing-fill",
                "absolute-in-auto-layout",
                "placeholder-text",
                "contrast",
                "low-contrast-text",
                "off-scale",
                "inconsistent-spacing",
                "non-scale-font-size",
                "inconsistent-radius",
                "hardcoded-value",
                "hardcoded-color",
                "hardcoded-number",
                "unstyled-text",
                "detached-instance",
                "missing-auto-layout",
                "tap-target-size",
            ] {
                println!("{rule_id}");
            }
            return Ok(());
        }
        (Some(flag), None, None) if flag == "--help" => {
            eprintln!("designos-mcp --workspace DIRECTORY\nLocal MCP server over stdio; edits are isolated until explicitly saved to a new file.");
            return Ok(());
        }
        _ => return Err("Usage: designos-mcp --workspace DIRECTORY".into()),
    };
    let service = DesignOSServer::new(&root)?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
