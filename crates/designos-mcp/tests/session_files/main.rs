//! A link or a FIFO planted in the session folder. designos-mcp writes nothing through it, reads nothing from it, never
//! waits on it, and tells the agent which path to clean up.
#![cfg(unix)]

mod fifos;
mod hazards;
mod ledger;
mod links;
mod options;
mod reports;
mod scratch;
mod stdio;
