use std::collections::BTreeMap;

#[derive(Debug)]
pub enum ErrorKind {
    MountsFileDoesNotExists,
    MountsPermissionDenied,
    MountsIsEmpty,
    UnknownError,
    RootNotMounted,
}

const PSEUDOFS: [&str; 44] = [
    // pseudofs will be ignored
    "anon_inodefs",
    "apparmorfs",
    "autofs",
    "bdev",
    "binder",
    "binfmt_misc",
    "bpf",
    "cgroup",
    "cgroup2",
    "configfs",
    "cpuset",
    "debugfs",
    "devfs",
    "devpts",
    "devtmpfs",
    "dlmfs",
    "dmabuf",
    "drm",
    "efivarfs",
    "esdfs",
    "hugetlbfs",
    "ipathfs",
    "mqueue",
    "nfsd",
    "none",
    "nsfs",
    "overlay",
    "pipefs",
    "proc",
    "pstore",
    "ramfs",
    "resctrl",
    "rootfs",
    "rpc_pipefs",
    "securityfs",
    "selinuxfs",
    "smackfs",
    "sockfs",
    "spufs",
    "sysfs",
    "tmpfs",
    "tracefs",
    "vboxsf",
    "virtiofs",
];

fn get_disk_info(what: &str) -> BTreeMap<String, String> {
    let mut result: BTreeMap<String, String> = BTreeMap::new();

    match std::fs::read_dir(format!("/dev/disk/by-{}", what.to_ascii_lowercase())) {
        Ok(dir_content) => {
            dir_content.for_each(|item| {
                if let Ok(item) = item {
                    result.insert(
                        std::fs::read_link(item.path())
                            .unwrap()
                            .as_path()
                            .to_str()
                            .unwrap()
                            .to_string()
                            .replace("../..", "/dev"),
                        item.file_name().into_string().unwrap(),
                    );
                }
            });
        }
        Err(_) => {
            return result;
        }
    }

    result
}

pub fn unmangle(s: &str) -> String {
    let mut result = s.to_string();
    [
        (r"\040", r" "),
        (r"\011", r"\t"),
        (r"\012", r"\n"),
        (r"\134", r"\\"),
        (r"\043", r"#"),
    ]
    .iter()
    .for_each(|o| result = result.replace(o.0, o.1));
    result
}

fn gen_swap() -> Option<String> {
    match std::fs::read_to_string("/proc/swaps") {
        Ok(content) => {
            let swp = match content.trim().split("\n").nth(1) {
                Some(line) => match line.split(" ").next() {
                    Some(path) => path,
                    None => "",
                },
                None => "",
            };
            if swp == "" {
                return None;
            }
            Some(format!(
                "{} none swap default{} 0 0",
                swp,
                if swp.starts_with("/dev/zram") {
                    ",pri=100"
                } else {
                    ""
                }
            ))
        }
        Err(_) => None,
    }
}

pub fn gen_from_mounts(root: &str, enable_uuid: bool) -> Result<Vec<String>, ErrorKind> {
    let mut result: Vec<String> = Vec::new();
    let uuids = get_disk_info("uuid");
    let mut written_drives: Vec<String> = Vec::new();
    match std::fs::read_to_string("/proc/mounts") {
        Ok(content) => {
            let content = content.trim();
            if content.is_empty() {
                return Err(ErrorKind::MountsIsEmpty);
            } else {
                let mut content: Vec<String> = content.split("\n").map(|s| s.to_string()).collect();
                match gen_swap() {
                    Some(swap) => content.push(swap),
                    None => {}
                }
                for line in content {
                    let mut splitted_line = line.split(" ");
                    let fs_spec = unmangle(splitted_line.next().unwrap());
                    let fs_file = unmangle(splitted_line.next().unwrap());
                    let fs_vfstype = unmangle(splitted_line.next().unwrap());
                    let fs_mntopts = unmangle(splitted_line.next().unwrap());

                    if PSEUDOFS.contains(&fs_vfstype.as_str())
                        || (fs_file.contains("/") && !fs_file.contains(root))
                        || fs_vfstype.starts_with("fuse")
                        || fs_spec.starts_with("/dev/loop")
                    {
                        continue;
                    }

                    let fs_freq = "0".to_string();
                    let fs_passno: String;

                    if fs_file == "none" {
                        fs_passno = "0".to_string();
                    } else if fs_file == root {
                        fs_passno = "1".to_string();
                    } else {
                        fs_passno = "2".to_string();
                    }

                    if !written_drives.contains(&fs_spec) {
                        result.push(format!(
                            "{} {} {} {} {} {}",
                            if enable_uuid && uuids.contains_key(&fs_spec) {
                                format!("UUID={}", uuids.get(&fs_spec).unwrap())
                            } else {
                                fs_spec.clone()
                            },
                            fs_file,
                            fs_vfstype,
                            fs_mntopts,
                            fs_freq,
                            fs_passno
                        ));

                        written_drives.push(fs_spec);
                    }
                }
            }
        }
        Err(error) => {
            return Err(match error.kind() {
                std::io::ErrorKind::NotFound => ErrorKind::MountsFileDoesNotExists,
                std::io::ErrorKind::PermissionDenied => ErrorKind::MountsPermissionDenied,
                _ => ErrorKind::UnknownError,
            });
        }
    };
    if result.is_empty() {
        return Err(ErrorKind::RootNotMounted);
    }
    Ok(result)
}
