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
            Arg::new("acp")
                .long("acp")
                .help("Run as ACP (Agent Client Protocol) server")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("mcp-server")
                .short('m')
                .long("mcp-server")
                .value_name("COMMAND")
                .help("MCP server command (format: 'command arg1 arg2')")
                .action(clap::ArgAction::Append),
        )
        .arg(
            Arg::new("engine")
                .short('e')
                .long("engine")
                .value_name("ENGINE")
                .help(
                    "Language engine to use: 'print' for console output, 'gemini' for AI responses",
                )
                .default_value("print"),
        )
        .arg(
            Arg::new("with-default-functions")
                .long("with-default-functions")
                .help("Include default functions (input, print)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("check")
                .long("check")
                .help("Parse, typecheck and run analyzers without executing the program")
                .action(clap::ArgAction::SetTrue),
        )
}
