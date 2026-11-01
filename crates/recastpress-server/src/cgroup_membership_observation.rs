//! Limited, read-only discovery of the visible cgroup v2 ancestry of self.
//!
//! Only one cgroup2 mount, with namespace-relative root `/`, is supported.
//! Namespace-external ancestors are always unknown, even when every visible
//! limit is `max`. These non-atomic observations are not capacity, headroom,
//! process RSS, allocation permissions or cache policy. No controller is changed.
//!
//! Before/after proc contents and namespace identities detect some changes;
//! symlink and overmount checks reject detected path substitutions. Standard
//! path-based opens cannot exclude TOCTOU, mount races or migration ABA, prove
//! cgroupfs authenticity, or prove initial namespace membership. A trusted,
//! stable proc/mount environment remains a prerequisite. Native Linux behavior
//! needs separate validation; portable fixtures are not that validation.

use crate::cgroup_memory_observation::CgroupV2LevelObservation;
use std::{io, path::PathBuf};

#[cfg(any(target_os = "linux", test))]
use std::{fs, io::Read, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpperAncestors {
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationConsistency {
    NonAtomic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CgroupV2LevelState {
    Observed(CgroupV2LevelObservation),
    /// All three memory files were absent at the visible mount boundary.
    /// This neither identifies the global root nor denotes unlimited memory.
    BoundaryUnobserved,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleCgroupV2Level {
    pub directory: PathBuf,
    pub state: CgroupV2LevelState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibleCgroupV2Observation {
    /// Raw namespace-relative path from the unified membership record.
    pub membership: Vec<u8>,
    /// Decoded raw mountpoint bytes from mountinfo, not a global identity.
    pub mountpoint: Vec<u8>,
    /// Leaf first, inclusive visible mount boundary last.
    pub levels: Vec<VisibleCgroupV2Level>,
    pub upper_ancestors: UpperAncestors,
    pub consistency: ObservationConsistency,
}

/// Observe a limited visible hierarchy. No effective memory allowance is derived.
#[cfg(target_os = "linux")]
pub fn observe_self_cgroup_v2() -> io::Result<VisibleCgroupV2Observation> {
    use std::os::unix::ffi::OsStrExt;
    observe_with(proc_snapshot, |plan| {
        let boundary = Path::new(std::ffi::OsStr::from_bytes(&plan.mountpoint));
        let mut leaf = boundary.to_path_buf();
        for part in plan
            .membership
            .split(|&b| b == b'/')
            .filter(|p| !p.is_empty())
        {
            leaf.push(std::ffi::OsStr::from_bytes(part));
        }
        read_ancestry(&leaf, boundary)
    })
}

#[cfg(any(target_os = "linux", test))]
fn observe_with(
    mut snapshot: impl FnMut() -> io::Result<ProcSnapshot>,
    read_levels: impl FnOnce(&Plan) -> io::Result<Vec<VisibleCgroupV2Level>>,
) -> io::Result<VisibleCgroupV2Observation> {
    let before = snapshot()?;
    let plan = resolve(&before.membership, &before.mountinfo)?;
    let levels = read_levels(&plan)?;
    unchanged(&before, &snapshot()?)?;
    Ok(VisibleCgroupV2Observation {
        membership: plan.membership,
        mountpoint: plan.mountpoint,
        levels,
        upper_ancestors: UpperAncestors::Unknown,
        consistency: ObservationConsistency::NonAtomic,
    })
}

#[cfg(not(target_os = "linux"))]
pub fn observe_self_cgroup_v2() -> io::Result<VisibleCgroupV2Observation> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "self cgroup v2 discovery is currently implemented only for Linux",
    ))
}

#[cfg(any(target_os = "linux", test))]
const MAX_DEPTH: usize = 128;
#[cfg(any(target_os = "linux", test))]
const MAX_PATH: usize = 4096;
#[cfg(any(target_os = "linux", test))]
const MAX_MEMBERSHIP: usize = 64 * 1024;
#[cfg(any(target_os = "linux", test))]
const MAX_MOUNTINFO: usize = 1024 * 1024;

#[cfg(any(target_os = "linux", test))]
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(any(target_os = "linux", test))]
fn bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(invalid("cgroup observation input exceeds its bound"));
    }
    Ok(bytes)
}

#[cfg(any(target_os = "linux", test))]
fn clean_path(path: &[u8]) -> io::Result<()> {
    if path.is_empty() || path.len() > MAX_PATH || path[0] != b'/' {
        return Err(invalid("cgroup path is not a bounded absolute path"));
    }
    if path == b"/" {
        return Ok(());
    }
    let mut depth = 0;
    for part in path[1..].split(|&b| b == b'/') {
        depth += 1;
        if part.is_empty()
            || part == b"."
            || part == b".."
            || part.iter().any(|b| *b < 0x20 || *b == 0x7f || *b == b'\\')
        {
            return Err(invalid("unsupported cgroup path component"));
        }
    }
    if depth > MAX_DEPTH {
        return Err(invalid("cgroup path exceeds ancestor depth bound"));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn decode_mount_path(input: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'\\' {
            let escaped = input
                .get(i + 1..i + 4)
                .ok_or_else(|| invalid("short mount path escape"))?;
            out.push(match escaped {
                b"040" => b' ',
                b"011" => b'\t',
                b"012" => b'\n',
                b"134" => b'\\',
                _ => return Err(invalid("unsupported mount path escape")),
            });
            i += 4;
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    clean_path(&out)?;
    Ok(out)
}

#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Eq, PartialEq)]
struct Plan {
    membership: Vec<u8>,
    mountpoint: Vec<u8>,
}

#[cfg(any(target_os = "linux", test))]
fn component_prefix(parent: &[u8], child: &[u8]) -> bool {
    parent == b"/"
        || child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|s| s.starts_with(b"/"))
}

#[cfg(any(target_os = "linux", test))]
fn resolve(membership: &[u8], mountinfo: &[u8]) -> io::Result<Plan> {
    if membership.len() > MAX_MEMBERSHIP || mountinfo.len() > MAX_MOUNTINFO {
        return Err(invalid("proc observation exceeds its bound"));
    }
    let mut member = None;
    for line in membership.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let mut fields = line.splitn(3, |&b| b == b':');
        let id = fields.next().unwrap_or_default();
        let controllers = fields
            .next()
            .ok_or_else(|| invalid("malformed membership record"))?;
        let path = fields
            .next()
            .ok_or_else(|| invalid("malformed membership record"))?;
        if id.is_empty() || !id.iter().all(u8::is_ascii_digit) {
            return Err(invalid("malformed membership hierarchy ID"));
        }
        if id == b"0" {
            if !controllers.is_empty() || member.is_some() {
                return Err(invalid("ambiguous unified membership"));
            }
            clean_path(path)?;
            member = Some(path.to_vec());
        }
    }
    let membership = member.ok_or_else(|| invalid("unified membership is absent"))?;
    let mut mounts = Vec::new();
    let mut chosen = None;
    let mut count = 0;
    for line in mountinfo.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        count += 1;
        if count > 4096 || line.len() > 16 * 1024 {
            return Err(invalid("mountinfo record bound exceeded"));
        }
        let fields: Vec<_> = line.split(|&b| b == b' ').collect();
        let sep = fields
            .iter()
            .position(|f| *f == b"-")
            .ok_or_else(|| invalid("mountinfo separator absent"))?;
        if sep < 6 || fields.len() != sep + 4 || fields.iter().any(|f| f.is_empty()) {
            return Err(invalid("malformed mountinfo fields"));
        }
        let root = decode_mount_path(fields[3])?;
        let mountpoint = decode_mount_path(fields[4])?;
        if fields[sep + 1] == b"cgroup2" {
            if chosen.is_some() || root != b"/" {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "ambiguous cgroup2 mount or unsupported mount root",
                ));
            }
            chosen = Some(mountpoint.clone());
        }
        mounts.push(mountpoint);
    }
    let mountpoint =
        chosen.ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "cgroup2 mount absent"))?;
    let mut leaf = mountpoint.clone();
    if membership != b"/" {
        if leaf == b"/" {
            leaf.clear();
        }
        leaf.extend_from_slice(&membership);
    }
    if leaf.len() > MAX_PATH {
        return Err(invalid("resolved cgroup path exceeds byte bound"));
    }
    // Reject all submounts, including bind mounts of individual memory files.
    let overlap = mounts.iter().filter(|p| **p == mountpoint).count() != 1
        || mounts
            .iter()
            .any(|p| p != &mountpoint && component_prefix(&mountpoint, p));
    if overlap {
        return Err(invalid("detected mount overlap on cgroup observation path"));
    }
    Ok(Plan {
        membership,
        mountpoint,
    })
}

#[cfg(any(target_os = "linux", test))]
#[derive(Eq, PartialEq)]
struct ProcSnapshot {
    membership: Vec<u8>,
    mountinfo: Vec<u8>,
    // Device/inode identities of cgroup and mount namespace handles.
    namespaces: [(u64, u64); 2],
}

#[cfg(target_os = "linux")]
fn proc_snapshot() -> io::Result<ProcSnapshot> {
    use std::os::unix::fs::MetadataExt;
    let identity = |path| {
        let metadata = fs::metadata(path)?;
        Ok::<_, io::Error>((metadata.dev(), metadata.ino()))
    };
    Ok(ProcSnapshot {
        membership: bounded(fs::File::open("/proc/self/cgroup")?, MAX_MEMBERSHIP)?,
        mountinfo: bounded(fs::File::open("/proc/self/mountinfo")?, MAX_MOUNTINFO)?,
        namespaces: [
            identity("/proc/self/ns/cgroup")?,
            identity("/proc/self/ns/mnt")?,
        ],
    })
}

#[cfg(any(target_os = "linux", test))]
fn unchanged(before: &ProcSnapshot, after: &ProcSnapshot) -> io::Result<()> {
    if before != after {
        return Err(invalid(
            "cgroup membership, mounts or namespace identity changed during observation",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn reject_symlinks(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            return Err(invalid("symlink in cgroup observation path"));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn read_token(path: &Path) -> io::Result<Vec<u8>> {
    reject_symlinks(path)?;
    bounded(fs::File::open(path)?, 128)
}

#[cfg(any(target_os = "linux", test))]
fn parse_memory(input: &[u8], allow_max: bool) -> io::Result<Option<u64>> {
    let token = input.strip_suffix(b"\n").unwrap_or(input);
    if allow_max && token == b"max" {
        return Ok(None);
    }
    if token.is_empty() {
        return Err(invalid("missing cgroup memory value"));
    }
    let mut value = 0u64;
    for &b in token {
        if !b.is_ascii_digit() {
            return Err(invalid("invalid cgroup memory value"));
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(b - b'0')))
            .ok_or_else(|| invalid("cgroup memory overflow"))?;
    }
    Ok(Some(value))
}

#[cfg(any(target_os = "linux", test))]
fn read_ancestry(leaf: &Path, boundary: &Path) -> io::Result<Vec<VisibleCgroupV2Level>> {
    if !leaf.starts_with(boundary) {
        return Err(invalid("cgroup leaf is outside mount boundary"));
    }
    let mut directory = leaf.to_path_buf();
    let mut levels = Vec::new();
    loop {
        if levels.len() > MAX_DEPTH {
            return Err(invalid("cgroup ancestry exceeds depth bound"));
        }
        reject_symlinks(&directory)?;
        let results = ["memory.current", "memory.max", "memory.high"]
            .map(|name| read_token(&directory.join(name)));
        let missing = results
            .iter()
            .filter(|r| {
                r.as_ref()
                    .is_err_and(|e| e.kind() == io::ErrorKind::NotFound)
            })
            .count();
        let state = if directory == boundary && missing == 3 {
            CgroupV2LevelState::BoundaryUnobserved
        } else {
            let [current, max, high] = results;
            let current = current?;
            let max = max?;
            let high = high?;
            let kind = read_token(&directory.join("cgroup.type"))?;
            if kind != b"domain\n" && kind != b"domain" {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "only domain cgroups are supported",
                ));
            }
            CgroupV2LevelState::Observed(CgroupV2LevelObservation {
                current_bytes: parse_memory(&current, false)?
                    .ok_or_else(|| invalid("memory.current cannot be max"))?,
                max_bytes: parse_memory(&max, true)?,
                high_bytes: parse_memory(&high, true)?,
            })
        };
        levels.push(VisibleCgroupV2Level {
            directory: directory.clone(),
            state,
        });
        if directory == boundary {
            break;
        }
        if !directory.pop() {
            return Err(invalid("cgroup boundary was not reached"));
        }
    }
    for level in &levels {
        reject_symlinks(&level.directory)?;
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const MOUNT: &[u8] = b"22 1 0:20 / /sys/fs/cgroup rw shared:7 future:1 - cgroup2 cgroup rw\n";

    #[test]
    fn resolves_bounded_membership_and_mountinfo_without_lossy_paths() {
        let plan = resolve(b"3:cpu:/v1\n0::/team/\xff\n", MOUNT).unwrap();
        assert_eq!(plan.membership, b"/team/\xff");
        assert_eq!(plan.mountpoint, b"/sys/fs/cgroup");
        assert_eq!(
            decode_mount_path(b"/space\\040name").unwrap(),
            b"/space name"
        );
        for input in [
            b"/bad\\0".as_slice(),
            b"/bad\\777",
            b"/a\\011b",
            b"/a\\134b",
        ] {
            assert!(decode_mount_path(input).is_err());
        }
        for member in [
            b"0::/../x\n".as_slice(),
            b"0::/a//b\n",
            b"0::/a/\n",
            b"0::/a\\040b\n",
            b"0::/a\n0::/a\n",
            b"0:cpu:/a\n",
            b"0::/a\nextra",
            b"1:cpu:/a\n",
        ] {
            assert!(resolve(member, MOUNT).is_err());
        }
        let deep = format!("0::/{}\n", vec!["a"; MAX_DEPTH + 1].join("/"));
        assert!(resolve(deep.as_bytes(), MOUNT).is_err());
        assert!(resolve(&vec![b'x'; MAX_MEMBERSHIP + 1], MOUNT).is_err());
        assert!(bounded(&b"abcd"[..], 3).is_err());
        assert_eq!(bounded(&b"abc"[..], 3).unwrap(), b"abc");
    }

    #[test]
    fn rejects_ambiguous_nonroot_and_overmounted_hierarchies() {
        for extra in [
            b"23 1 0:20 / /other rw - cgroup2 cgroup rw\n".as_slice(),
            b"23 22 1:1 / /sys/fs/cgroup/team rw - tmpfs tmpfs rw\n",
            b"23 22 1:1 / /sys/fs/cgroup rw - tmpfs tmpfs rw\n",
            b"23 22 1:1 / /sys/fs/cgroup/team/leaf/memory.max rw - tmpfs tmpfs rw\n",
        ] {
            let mut mounts = MOUNT.to_vec();
            mounts.extend_from_slice(extra);
            assert!(resolve(b"0::/team/leaf\n", &mounts).is_err());
        }
        assert!(
            resolve(
                b"0::/team\n",
                b"22 1 0:20 /delegated /cg rw - cgroup2 cg rw\n"
            )
            .is_err()
        );
        // A textual sibling of the mountpoint is not inside its subtree.
        let mut mounts = MOUNT.to_vec();
        mounts.extend_from_slice(b"23 22 1:1 / /sys/fs/cgroup-other rw - tmpfs tmpfs rw\n");
        assert!(resolve(b"0::/team/leaf\n", &mounts).is_ok());
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "recastpress-cgroup-membership-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("leaf")).unwrap();
            Self(path)
        }
        fn populate(&self, sub: &str) {
            for (name, value) in [
                ("memory.current", "20\n"),
                ("memory.max", "max\n"),
                ("memory.high", "0\n"),
                ("cgroup.type", "domain\n"),
            ] {
                fs::write(self.0.join(sub).join(name), value).unwrap();
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only fixed files/directories created by this fixture, no recursion.
            for sub in ["leaf", ""] {
                for name in ["memory.current", "memory.max", "memory.high", "cgroup.type"] {
                    let _ = fs::remove_file(self.0.join(sub).join(name));
                }
            }
            let _ = fs::remove_dir(self.0.join("leaf"));
            let _ = fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn owned_files_preserve_boundary_missing_and_max_distinction() {
        let fixture = Fixture::new();
        let leaf = fixture.0.join("leaf");
        // Three absent memory files are special only at the visible boundary,
        // never at the process leaf below it.
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        fixture.populate("leaf");
        let levels = read_ancestry(&leaf, &fixture.0).unwrap();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0].directory, leaf);
        assert_eq!(
            levels[0].state,
            CgroupV2LevelState::Observed(CgroupV2LevelObservation {
                current_bytes: 20,
                max_bytes: None,
                high_bytes: Some(0)
            })
        );
        assert_eq!(levels[1].state, CgroupV2LevelState::BoundaryUnobserved);
        fs::write(fixture.0.join("memory.max"), "max\n").unwrap();
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        // Two absent boundary files plus a malformed third are not the
        // all-NotFound boundary state either.
        fs::write(fixture.0.join("memory.max"), "invalid\n").unwrap();
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        fixture.populate("");
        fs::write(fixture.0.join("memory.high"), "max\n").unwrap();
        assert!(matches!(
            read_ancestry(&leaf, &fixture.0).unwrap()[1].state,
            CgroupV2LevelState::Observed(_)
        ));
        fs::remove_file(leaf.join("memory.current")).unwrap();
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        fixture.populate("leaf");
        fs::write(leaf.join("cgroup.type"), "threaded\n").unwrap();
        assert_eq!(
            read_ancestry(&leaf, &fixture.0).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        fixture.populate("leaf");
        fs::write(leaf.join("memory.current"), "max\n").unwrap();
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        fs::write(leaf.join("memory.current"), vec![b'0'; 129]).unwrap();
        assert!(read_ancestry(&leaf, &fixture.0).is_err());
        for token in [
            b"+1".as_slice(),
            b"-1",
            b"1 kB",
            b"1.0",
            b"1\n\n",
            b"18446744073709551616",
            b"\xff",
        ] {
            assert!(parse_memory(token, true).is_err());
        }
        assert_eq!(
            parse_memory(b"18446744073709551615\n", false).unwrap(),
            Some(u64::MAX)
        );
    }

    #[test]
    fn injected_proc_reader_rejects_each_detected_change_and_keeps_unknown() {
        for changed in 0..4 {
            let mut calls = 0;
            let result = observe_with(
                || {
                    calls += 1;
                    let mut s = ProcSnapshot {
                        membership: b"0::/team\n".to_vec(),
                        mountinfo: MOUNT.to_vec(),
                        namespaces: [(1, 2), (3, 4)],
                    };
                    if calls == 2 {
                        match changed {
                            0 => s.membership = b"0::/other\n".to_vec(),
                            1 => s.mountinfo.push(b'\n'),
                            2 => s.namespaces[0].1 += 1,
                            _ => s.namespaces[1].0 += 1,
                        }
                    }
                    Ok(s)
                },
                |_| Ok(Vec::new()),
            );
            assert!(result.is_err());
            assert_eq!(calls, 2);
        }
        let result = observe_with(
            || {
                Ok(ProcSnapshot {
                    membership: b"0::/team\n".to_vec(),
                    mountinfo: MOUNT.to_vec(),
                    namespaces: [(1, 2), (3, 4)],
                })
            },
            |_| Ok(Vec::new()),
        )
        .unwrap();
        assert_eq!(result.upper_ancestors, UpperAncestors::Unknown);
        assert_eq!(result.consistency, ObservationConsistency::NonAtomic);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_is_unsupported() {
        assert_eq!(
            observe_self_cgroup_v2().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }

    // This test needs native Linux. Cross-compilation does not execute it.
    #[cfg(target_os = "linux")]
    #[test]
    fn rejects_detected_linux_symlink() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        fixture.populate("leaf");
        let path = fixture.0.join("leaf/memory.current");
        fs::remove_file(&path).unwrap();
        symlink("memory.max", &path).unwrap();
        assert!(read_ancestry(&fixture.0.join("leaf"), &fixture.0).is_err());
    }
}
