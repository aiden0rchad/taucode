use objc2_app_kit::{NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;
use std::{ffi::CStr, os::unix::ffi::OsStrExt, path::PathBuf, time::Duration};

pub(super) struct Binding {
    pub pid: i32,
    bundle: String,
    pub bundle_path: PathBuf,
    executable: PathBuf,
}

impl Binding {
    pub fn bind(pid: i32, bundle: &str) -> Result<Self, String> {
        let path = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .and_then(|app| app.bundleURL())
            .or_else(|| {
                NSWorkspace::sharedWorkspace()
                    .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle))
            })
            .and_then(|url| url.path())
            .ok_or_else(|| {
                format!("Process check (bind): pid={pid}, bundle={bundle}, bundle URL unavailable")
            })?;
        let bundle_path = std::fs::canonicalize(path.to_string())
            .map_err(|e| format!("Process check (bind): pid={pid}, bundle path={path}, {e}"))?;
        let executable =
            kernel_path(pid).map_err(|e| format!("Process check (bind): pid={pid}, {e}"))?;
        let binding = Self {
            pid,
            bundle: bundle.into(),
            bundle_path,
            executable,
        };
        binding.check("bind")?;
        Ok(binding)
    }

    pub fn check(&self, site: &str) -> Result<(), String> {
        self.check_with(
            site,
            || kernel_path(self.pid),
            || {
                NSRunningApplication::runningApplicationWithProcessIdentifier(self.pid).map(|app| {
                    (
                        app.isTerminated(),
                        app.bundleIdentifier().map(|id| id.to_string()),
                    )
                })
            },
            std::thread::sleep,
        )
    }

    fn check_with(
        &self,
        site: &str,
        mut kernel: impl FnMut() -> Result<PathBuf, String>,
        mut application: impl FnMut() -> Option<(bool, Option<String>)>,
        mut wait: impl FnMut(Duration),
    ) -> Result<(), String> {
        for attempt in 0..2 {
            let path =
                kernel().map_err(|e| format!("Process check ({site}): pid={}, {e}", self.pid))?;
            if !path.starts_with(&self.bundle_path) || path != self.executable {
                return Err(format!(
                    "Process binding changed ({site}): pid={}, path={}, expected bundle={}, expected executable={}",
                    self.pid, path.display(), self.bundle_path.display(), self.executable.display()
                ));
            }
            match application() {
                None | Some((true, _)) if attempt == 0 => wait(Duration::from_millis(100)),
                // AppKit can lag behind the kernel on background threads.
                None | Some((true, _)) => return Ok(()),
                Some((false, bundle)) => {
                    if bundle.as_ref().is_some_and(|id| id != &self.bundle) {
                        return Err(format!(
                            "Process binding changed ({site}): pid={}, path={}, NSRunningApplication bundle={bundle:?}, expected bundle={}",
                            self.pid, path.display(), self.bundle
                        ));
                    }
                    return Ok(());
                }
            }
        }
        unreachable!()
    }
}

fn kernel_path(pid: i32) -> Result<PathBuf, String> {
    if pid <= 0 {
        return Err("invalid PID".into());
    }
    let kill = if unsafe { libc::kill(pid, 0) } == 0 {
        "0"
    } else {
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => return Err("kill=ESRCH (process exited)".into()),
            Some(libc::EPERM) => "EPERM", // The process exists; still verify its path.
            _ => return Err(format!("kill={error}")),
        }
    };
    let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    if unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) } <= 0 {
        return Err(format!(
            "kill={kill}, proc_pidpath={}",
            std::io::Error::last_os_error()
        ));
    }
    let path = CStr::from_bytes_until_nul(&buffer)
        .map_err(|e| format!("kill={kill}, proc_pidpath={e}"))?;
    std::fs::canonicalize(std::ffi::OsStr::from_bytes(path.to_bytes()))
        .map_err(|e| format!("kill={kill}, proc_pidpath={}, {e}", path.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            pid: 42,
            bundle: "test.app".into(),
            bundle_path: "/Applications/Test.app".into(),
            executable: "/Applications/Test.app/Contents/MacOS/Test".into(),
        }
    }

    #[test]
    fn kernel_identity_survives_missing_or_terminated_appkit_after_one_retry() {
        let b = binding();
        for app in [None, Some((true, Some("stale.bundle".into())))] {
            let mut probes = 0;
            let mut waits = vec![];
            b.check_with(
                "pre-flight capture",
                || {
                    probes += 1;
                    Ok(b.executable.clone())
                },
                || app.clone(),
                |d| waits.push(d),
            )
            .unwrap();
            assert_eq!(probes, 2);
            assert_eq!(waits, [Duration::from_millis(100)]);
        }
    }

    #[test]
    fn exited_or_rebound_process_fails_without_retry() {
        let b = binding();
        for (observation, expected) in [
            (Err("kill=ESRCH (process exited)".into()), "kill=ESRCH"),
            (
                Ok("/Applications/Other.app/Contents/MacOS/Test".into()),
                "Process binding changed",
            ),
            (
                Ok("/Applications/Test.app.evil/Contents/MacOS/Test".into()),
                "Process binding changed",
            ),
            (
                Ok("/Applications/Test.app/Contents/MacOS/Other".into()),
                "Process binding changed",
            ),
        ] {
            let error = b
                .check_with(
                    "pre-flight capture",
                    || observation.clone(),
                    || panic!("kernel failure must be immediate"),
                    |_| panic!("must not retry"),
                )
                .unwrap_err();
            assert!(error.contains(expected), "{error}");
            assert!(error.contains("pre-flight capture"), "{error}");
            assert!(error.contains("pid=42"), "{error}");
        }
    }

    #[test]
    fn retry_rechecks_kernel_identity() {
        let b = binding();
        for second in [Err("kill=ESRCH".into()), Ok("/other/app".into())] {
            let mut observations = [Ok(b.executable.clone()), second].into_iter();
            assert!(b
                .check_with("retry", || observations.next().unwrap(), || None, |_| {})
                .is_err());
        }
    }

    #[test]
    fn live_appkit_bundle_mismatch_fails_without_retry() {
        let b = binding();
        let error = b
            .check_with(
                "session",
                || Ok(b.executable.clone()),
                || Some((false, Some("other.app".into()))),
                |_| panic!("must not retry"),
            )
            .unwrap_err();
        assert!(error.contains("Process binding changed"));
        assert!(error.contains("other.app"));
    }
}
