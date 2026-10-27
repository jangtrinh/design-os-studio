use designos_mcp::server::DesignOSServer;
use rmcp::{transport::stdio, ServiceExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = match (args.next(), args.next(), args.next()) {
        (Some(flag), Some(path), None) if flag == "--workspace" => std::path::PathBuf::from(path),
        (Some(flag), None, None) if flag == "--list-lint-rules" => {
            // One canonical registry id per line, alphabetical so the listing is the same on every run.
            // Aliases are accepted by the linter but never listed.
            let mut rule_ids = designos_doc::lint::all_rule_ids();
            rule_ids.sort_unstable();
            for rule_id in rule_ids {
                println!("{rule_id}");
            }
            return Ok(());
        }
        (Some(flag), None, None) if flag == "--version" || flag == "-V" => {
            println!("designos-mcp {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        (Some(flag), None, None) if flag == "--help" => {
            eprintln!(
                "designos-mcp --workspace DIRECTORY\nLocal MCP server over stdio; edits are isolated until explicitly saved to a new file.\n  --version, -V  print the version and exit"
            );
            return Ok(());
        }
        _ => return Err("Usage: designos-mcp --workspace DIRECTORY | --version | --list-lint-rules | --help".into()),
    };
    let service = DesignOSServer::new(&root)?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
