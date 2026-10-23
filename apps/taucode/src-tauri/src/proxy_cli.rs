pub fn proxy_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Option<Vec<std::ffi::OsString>> {
    let mut args = args.into_iter().skip(1);
    (args.next().as_deref() == Some(std::ffi::OsStr::new("--tau-mcp-proxy")))
        .then(|| args.collect())
}

pub fn dispatch() {
    if let Some(args) = proxy_args(std::env::args_os()) {
        let status = match tau_mcp_proxy::run_cli(args) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error:#}");
                1
            }
        };
        std::process::exit(status);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dispatch_before_host_initialization() {
        let args = ["app", "--tau-mcp-proxy", "--", "upstream"].map(Into::into);
        assert_eq!(super::proxy_args(args).unwrap(), ["--", "upstream"]);
        assert!(super::proxy_args(["app", "--other"].map(Into::into)).is_none());
    }
}
