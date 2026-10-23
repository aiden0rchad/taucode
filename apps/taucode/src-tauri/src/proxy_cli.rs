pub fn proxy_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Option<Vec<std::ffi::OsString>> {
    let mut args = args.into_iter().skip(1);
    (args.next().as_deref() == Some(std::ffi::OsStr::new("--tau-mcp-proxy")))
        .then(|| args.collect())
}

pub fn dispatch() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "--tau-browser-mcp") {
        let status = match codex_monitor_lib::browser_mcp_cli(args[2..].to_vec()) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
        std::process::exit(status);
    }
    if args.get(1).is_some_and(|s| s == "--tau-mcp-env") {
        match env_command(&args[2..]) {
            Ok(mut command) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    let _ = command.exec();
                    eprintln!("Cannot start connector command");
                    std::process::exit(1);
                }
                #[cfg(not(unix))]
                std::process::exit(command.status().ok().and_then(|s| s.code()).unwrap_or(1));
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
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

fn env_command(args: &[std::ffi::OsString]) -> Result<std::process::Command, &'static str> {
    let mapping: std::collections::BTreeMap<String, String> = serde_json::from_str(
        args.first()
            .and_then(|s| s.to_str())
            .ok_or("Missing connector environment mapping")?,
    )
    .map_err(|_| "Invalid connector environment mapping")?;
    let mut command = std::process::Command::new(args.get(1).ok_or("Missing connector command")?);
    command.args(&args[2..]);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("TAUCODE_MCP_") {
            command.env_remove(name);
        }
    }
    for (name, reference) in mapping {
        if name.is_empty()
            || name.starts_with(|c: char| c.is_ascii_digit())
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || !reference.starts_with("TAUCODE_MCP_")
            || !reference
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("Invalid connector environment mapping");
        }
        command.env(
            name,
            std::env::var_os(reference).ok_or("Missing connector credential")?,
        );
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    #[test]
    fn environment_launcher_rejects_arbitrary_references() {
        assert!(super::env_command(&[r#"{"KEY":"HOME"}"#.into(), "server".into()]).is_err());
        let command =
            super::env_command(&["{}".into(), "server".into(), "--stdio".into()]).unwrap();
        assert_eq!(command.get_program(), "server");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["--stdio"]);
    }
    #[test]
    fn dispatch_before_host_initialization() {
        let args = ["app", "--tau-mcp-proxy", "--", "upstream"].map(Into::into);
        assert_eq!(super::proxy_args(args).unwrap(), ["--", "upstream"]);
        assert!(super::proxy_args(["app", "--other"].map(Into::into)).is_none());
    }
}
