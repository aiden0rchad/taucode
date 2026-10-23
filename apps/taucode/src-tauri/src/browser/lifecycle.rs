use serde::{Deserialize, Serialize};
use std::{io, os::unix::ffi::OsStrExt, path::Path, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Owner {
    host: String,
    pid: i32,
    group: Option<i32>,
}

fn singleton(text: &str) -> Option<Owner> {
    let (host, pid) = text.rsplit_once('-')?;
    let pid = pid.parse::<i32>().ok().filter(|p| *p > 1)?;
    (!host.is_empty() && !host.ends_with('-') && !host.contains('/')).then(|| Owner {
        host: host.into(),
        pid,
        group: None,
    })
}

fn hostname() -> io::Result<String> {
    let mut name = [0u8; 256];
    if unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let end = name
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| io::Error::other("Hostname too long"))?;
    Ok(String::from_utf8_lossy(&name[..end]).into())
}

pub(super) fn record(data: &Path, pid: u32) -> io::Result<()> {
    let owner = Owner {
        host: hostname()?,
        pid: pid as i32,
        group: Some(pid as i32),
    };
    let path = data.join("browser/process.json");
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec(&owner)?)?;
    std::fs::rename(temporary, path)
}

#[derive(Clone)]
struct Info {
    pid: i32,
    parent: i32,
    group: i32,
    uid: u32,
    zombie: bool,
}

trait Processes {
    fn list(&mut self) -> io::Result<Vec<Info>>;
    fn arguments(&mut self, pid: i32) -> io::Result<Vec<Vec<u8>>>;
    // Negative targets address a process group, positive targets a single process.
    fn signal(&mut self, target: i32, signal: i32) -> io::Result<()>;
    fn pause(&mut self) -> impl std::future::Future<Output = ()> + Send;
}
struct System;
impl Processes for System {
    fn list(&mut self) -> io::Result<Vec<Info>> {
        let output = std::process::Command::new("/bin/ps")
            .args(["-axo", "pid=,ppid=,pgid=,uid=,stat="])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("Cannot inspect browser processes"));
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| {
                let mut fields = line.split_whitespace();
                let mut number = || {
                    fields
                        .next()
                        .and_then(|v| v.parse::<i32>().ok())
                        .ok_or_else(|| io::Error::other("Invalid ps output"))
                };
                let (pid, parent, group, uid) = (number()?, number()?, number()?, number()? as u32);
                Ok(Info {
                    pid,
                    parent,
                    group,
                    uid,
                    zombie: fields.next().is_some_and(|s| s.starts_with('Z')),
                })
            })
            .collect()
    }
    fn arguments(&mut self, pid: i32) -> io::Result<Vec<Vec<u8>>> {
        arguments(pid)
    }
    fn signal(&mut self, target: i32, signal: i32) -> io::Result<()> {
        if unsafe { libc::kill(target, signal) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(e)
        }
    }
    async fn pause(&mut self) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(target_os = "linux")]
fn arguments(pid: i32) -> io::Result<Vec<Vec<u8>>> {
    Ok(std::fs::read(format!("/proc/{pid}/cmdline"))?
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(<[u8]>::to_vec)
        .collect())
}
#[cfg(target_os = "macos")]
fn arguments(pid: i32) -> io::Result<Vec<Vec<u8>>> {
    // KERN_PROCARGS2 returns argc, executable path, padding, then NUL-separated argv.
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut bytes = vec![0u8; 1024 * 1024];
    let mut size = bytes.len();
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    bytes.truncate(size);
    parse_arguments(&bytes)
}
#[cfg(target_os = "macos")]
fn parse_arguments(bytes: &[u8]) -> io::Result<Vec<Vec<u8>>> {
    let invalid = || io::Error::other("Invalid process arguments");
    let argc = i32::from_ne_bytes(bytes.get(..4).ok_or_else(invalid)?.try_into().unwrap());
    if argc <= 0 {
        return Err(invalid());
    }
    let tail = &bytes[4..];
    let path_end = tail.iter().position(|b| *b == 0).ok_or_else(invalid)?;
    let argv = &tail[path_end..];
    let start = argv.iter().position(|b| *b != 0).ok_or_else(invalid)?;
    let args: Vec<_> = argv[start..]
        .split(|b| *b == 0)
        .take(argc as usize)
        .map(<[u8]>::to_vec)
        .collect();
    if args.len() != argc as usize {
        return Err(invalid());
    }
    Ok(args)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn arguments(_: i32) -> io::Result<Vec<Vec<u8>>> {
    Err(io::Error::other(
        "Browser process inspection is unsupported on this platform",
    ))
}

fn targets(
    processes: &mut impl Processes,
    owners: &[Owner],
    profile: &Path,
    host: &str,
    uid: u32,
    own_pid: i32,
    own_group: i32,
) -> io::Result<Vec<i32>> {
    let all = processes.list()?;
    let mut expected = b"--user-data-dir=".to_vec();
    expected.extend_from_slice(profile.as_os_str().as_bytes());
    let mut roots = Vec::new();
    let mut groups = Vec::new();
    for owner in owners
        .iter()
        .filter(|o| o.host == host && o.pid > 1 && o.pid != own_pid)
    {
        let root = all.iter().find(|p| p.pid == owner.pid);
        // If Chrome's parent died, a recorded private group can still own renderers.
        let candidates: Vec<_> = all
            .iter()
            .filter(|p| {
                p.uid == uid
                    && !p.zombie
                    && (p.pid == owner.pid
                        || (root.is_none_or(|p| p.zombie)
                            && owner.group == Some(owner.pid)
                            && p.group == owner.pid))
            })
            .collect();
        for candidate in candidates {
            let args = match processes.arguments(candidate.pid) {
                Ok(args) => args,
                Err(e) if matches!(e.raw_os_error(), Some(libc::ESRCH | libc::ENOENT)) => continue,
                Err(e) => return Err(e),
            };
            if !args.iter().any(|a| a == &expected) {
                continue;
            } // PID reuse / unrelated process.
            roots.push(candidate.pid);
            if candidate.group == owner.pid && candidate.group != own_group && candidate.group > 1 {
                groups.push(candidate.group);
            }
        }
    }
    Ok(tree_targets(&all, roots, groups, uid, own_pid))
}

fn tree_targets(
    all: &[Info],
    mut roots: Vec<i32>,
    mut groups: Vec<i32>,
    uid: u32,
    own_pid: i32,
) -> Vec<i32> {
    // Include descendants even if a helper left Chrome's process group. Legacy
    // Chrome shared the daemon's group: kill that tree by PID, never the daemon.
    loop {
        let children: Vec<_> = all
            .iter()
            .filter(|p| {
                p.uid == uid
                    && p.pid > 1
                    && p.pid != own_pid
                    && !roots.contains(&p.pid)
                    && (roots.contains(&p.parent) || groups.contains(&p.group))
            })
            .map(|p| p.pid)
            .collect();
        if children.is_empty() {
            break;
        }
        roots.extend(children);
    }
    groups.sort_unstable();
    groups.dedup();
    let mut targets: Vec<_> = roots
        .into_iter()
        .rev()
        .filter(|pid| {
            !all.iter()
                .any(|p| p.pid == *pid && groups.contains(&p.group))
        })
        .collect();
    targets.extend(groups.into_iter().map(|g| -g));
    targets
}

async fn terminate(processes: &mut impl Processes, targets: &[i32]) -> io::Result<()> {
    let mut remaining = targets.to_vec();
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        for target in &remaining {
            processes.signal(*target, signal)?;
        }
        for _ in 0..40 {
            let all = processes.list()?;
            remaining.retain(|target| {
                all.iter().any(|p| {
                    !p.zombie
                        && if *target < 0 {
                            p.group == -*target
                        } else {
                            p.pid == *target
                        }
                })
            });
            if remaining.is_empty() {
                return Ok(());
            }
            processes.pause().await;
        }
    }
    Err(io::Error::other("Browser process tree did not exit"))
}

pub(super) async fn stop(data: &Path, pid: u32) -> io::Result<()> {
    let mut system = System;
    let all = system.list()?;
    let targets = tree_targets(
        &all,
        Vec::new(),
        vec![pid as i32],
        unsafe { libc::geteuid() },
        std::process::id() as i32,
    );
    terminate(&mut system, &targets).await?;
    match std::fs::remove_file(data.join("browser/process.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub(super) async fn cleanup(data: &Path) -> io::Result<()> {
    let mut owners = Vec::new();
    if let Ok(path) = std::fs::read_link(super::process::profile(data).join("SingletonLock")) {
        if let Some(owner) = path.to_str().and_then(singleton) {
            owners.push(owner);
        }
    }
    let record = data.join("browser/process.json");
    match std::fs::read(&record) {
        Ok(bytes) => owners.push(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    if !owners.is_empty() {
        let mut system = System;
        let targets = targets(
            &mut system,
            &owners,
            &super::process::profile(data),
            &hostname()?,
            unsafe { libc::geteuid() },
            std::process::id() as i32,
            unsafe { libc::getpgrp() },
        )?;
        terminate(&mut system, &targets).await?;
    }
    match std::fs::remove_file(record) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

// Last-resort cleanup for cancelled startup / failed CDP initialization. Normal
// shutdown uses stop above, including TERM, escalation and waiting.
pub(super) fn kill_group(pid: u32) {
    if pid > 1 && pid as i32 != unsafe { libc::getpgrp() } {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        processes: Vec<Info>,
        args: HashMap<i32, Vec<Vec<u8>>>,
        signals: Vec<(i32, i32)>,
        exits_on: i32,
        waits: usize,
    }
    impl Processes for Fake {
        fn list(&mut self) -> io::Result<Vec<Info>> {
            Ok(self.processes.clone())
        }
        fn arguments(&mut self, pid: i32) -> io::Result<Vec<Vec<u8>>> {
            Ok(self.args.get(&pid).cloned().unwrap_or_default())
        }
        fn signal(&mut self, target: i32, signal: i32) -> io::Result<()> {
            self.signals.push((target, signal));
            if signal == self.exits_on {
                self.processes.retain(|p| {
                    if target < 0 {
                        p.group != -target
                    } else {
                        p.pid != target
                    }
                });
            }
            Ok(())
        }
        async fn pause(&mut self) {
            self.waits += 1;
        }
    }
    fn process(pid: i32, parent: i32, group: i32) -> Info {
        Info {
            pid,
            parent,
            group,
            uid: 501,
            zombie: false,
        }
    }
    fn owner(pid: i32, group: Option<i32>) -> Owner {
        Owner {
            host: "my-mac.local".into(),
            pid,
            group,
        }
    }
    fn plan(fake: &mut Fake, owners: &[Owner]) -> Vec<i32> {
        targets(
            fake,
            owners,
            Path::new("/app data/browser/profile"),
            "my-mac.local",
            501,
            10,
            10,
        )
        .unwrap()
    }
    fn browser_args() -> Vec<Vec<u8>> {
        vec![
            b"Chrome".to_vec(),
            b"--user-data-dir=/app data/browser/profile".to_vec(),
        ]
    }

    #[test]
    fn singleton_lock_uses_last_hyphen_and_rejects_invalid_pids() {
        let parsed = singleton("my-mac.local-12345").unwrap();
        assert_eq!(parsed.host, "my-mac.local");
        assert_eq!(parsed.pid, 12345);
        for bad in [
            "",
            "host",
            "-23",
            "host-0",
            "host-1",
            "host--2",
            "host-nan",
            "host-2147483648",
            "/tmp/host-23",
        ] {
            assert!(singleton(bad).is_none(), "{bad}");
        }
    }
    #[test]
    fn private_group_includes_helpers_and_escaped_descendants() {
        let mut fake = Fake {
            processes: vec![
                process(20, 1, 20),
                process(21, 20, 20),
                process(22, 21, 22),
                process(10, 1, 10),
            ],
            ..Default::default()
        };
        fake.args.insert(20, browser_args());
        assert_eq!(plan(&mut fake, &[owner(20, Some(20))]), vec![22, -20]);
        assert_eq!(
            tree_targets(&fake.processes, Vec::new(), vec![20], 501, 10),
            vec![22, -20]
        );
    }
    #[test]
    fn legacy_browser_tree_does_not_kill_daemon_group() {
        let mut fake = Fake {
            processes: vec![
                process(10, 1, 10),
                process(20, 1, 10),
                process(21, 20, 10),
                process(22, 10, 10),
            ],
            ..Default::default()
        };
        fake.args.insert(20, browser_args());
        assert_eq!(plan(&mut fake, &[owner(20, None)]), vec![21, 20]);
    }
    #[test]
    fn stale_pid_needs_local_host_matching_uid_and_exact_profile() {
        let mut fake = Fake {
            processes: vec![process(20, 1, 20)],
            ..Default::default()
        };
        assert!(plan(&mut fake, &[owner(20, Some(20))]).is_empty());
        fake.args.insert(
            20,
            vec![b"--user-data-dir=/app data/browser/profile-other".to_vec()],
        );
        assert!(plan(&mut fake, &[owner(20, Some(20))]).is_empty());
        fake.args.insert(20, browser_args());
        let mut remote = owner(20, Some(20));
        remote.host = "another-mac.local".into();
        assert!(plan(&mut fake, &[remote]).is_empty());
        fake.processes[0].uid = 502;
        assert!(plan(&mut fake, &[owner(20, Some(20))]).is_empty());
        fake.processes[0] = process(10, 1, 10);
        fake.args.insert(10, browser_args());
        assert!(plan(&mut fake, &[owner(10, Some(10))]).is_empty());
    }
    #[test]
    fn recorded_group_recovers_orphans_only_with_profile_evidence() {
        let mut fake = Fake {
            processes: vec![process(21, 1, 20)],
            ..Default::default()
        };
        assert!(plan(&mut fake, &[owner(20, Some(20))]).is_empty());
        fake.args.insert(21, browser_args());
        assert_eq!(plan(&mut fake, &[owner(20, Some(20))]), vec![-20]);
        assert!(plan(&mut fake, &[owner(20, None)]).is_empty());
    }
    #[tokio::test]
    async fn tree_shutdown_waits_and_escalates_only_when_needed() {
        for exits_on in [libc::SIGTERM, libc::SIGKILL, 0] {
            let mut fake = Fake {
                processes: vec![process(20, 1, 20), process(21, 20, 20)],
                exits_on,
                ..Default::default()
            };
            assert_eq!(terminate(&mut fake, &[-20]).await.is_ok(), exits_on != 0);
            assert_eq!(fake.signals[0], (-20, libc::SIGTERM));
            if exits_on == libc::SIGTERM {
                assert_eq!(fake.signals.len(), 1);
                assert_eq!(fake.waits, 0);
            } else {
                assert_eq!(fake.signals[1], (-20, libc::SIGKILL));
                assert!(fake.waits >= 40);
            }
        }
    }
    #[tokio::test]
    async fn zombies_do_not_block_waiting_for_tree_exit() {
        let mut zombie = process(20, 1, 20);
        zombie.zombie = true;
        let mut fake = Fake {
            processes: vec![zombie],
            ..Default::default()
        };
        terminate(&mut fake, &[-20]).await.unwrap();
        assert_eq!(fake.signals, vec![(-20, libc::SIGTERM)]);
        assert_eq!(fake.waits, 0);
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn procargs_preserves_spaces_and_excludes_environment() {
        let mut bytes = 2i32.to_ne_bytes().to_vec();
        bytes.extend_from_slice(b"/Applications/Google Chrome\0\0\0Chrome\0--user-data-dir=/app data/browser/profile\0SECRET=value\0");
        assert_eq!(parse_arguments(&bytes).unwrap(), browser_args());
        assert!(parse_arguments(&[]).is_err());
    }
}
