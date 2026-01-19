use clap::{Arg, Command};

pub fn build_cli() -> Command {
    Command::new("structured-agent")
        .version("0.1.0")
        .about("A structured agent runtime with MCP support")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .value_name("FILE")
                .help("Program file to execute")
                .conflicts_with("inline"),
        )
        .arg(
            Arg::new("inline")
                .short('i')
                .long("inline")
                .value_name("CODE")
                .help("Inline program code to execute")
                .conflicts_with("file"),
        )
        .arg(
            Arg::new("mcp-server")
                .short('m')
                .long("mcp-server")
                .value_name("COMMAND")
                .help("MCP server command (format: 'command arg1 arg2')")
                .action(clap::ArgAction::Append),
        )
}
