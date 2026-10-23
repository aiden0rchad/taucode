fn main() -> anyhow::Result<()> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--tau-mcp-proxy") {
        args.remove(0);
    }
    tau_mcp_proxy::run_cli(args)
}
