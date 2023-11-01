use genfstab;
use std::io::Write;

extern "C" {
    fn geteuid() -> u32; // user id
}

const HELP: &str = "USAGE: genfstab [OPTIONS] root_path
Options:
    -U           use UUID.
    --to-fstab   write result into /etc/fstab in root.";

fn root_etc_path(root: &str) -> String {
    format!("{}{}etc", root, if root.ends_with("/") { "" } else { "/" })
}

fn error(info: &str) {
    println!("{info}");
    std::process::exit(1);
}

fn ask_confirm(question: &str, default: bool) -> bool {
    print!("{}", question);
    std::io::stdout().flush().unwrap();

    let mut answer = String::new();
    let _ = std::io::stdin().read_line(&mut answer);
    answer = answer.trim().to_string();

    if answer.is_empty() {
        return default;
    }

    match answer.chars().next().unwrap() {
        'y' | 'Y' => return true,
        _ => return false,
    }
}

fn write_to_file(path: String, content: Vec<String>) {
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
    {
        Ok(mut file) => content.iter().for_each(|line| {
            let _ = write!(file, "{}\n\n", line);
        }),
        Err(error) => {
            panic!("{:?}", error.kind())
        }
    };
}

fn main() {
    let mut root: String = "".to_string();
    let args: Vec<String> = std::env::args().collect();
    if unsafe { geteuid() != 0 } {
        error(
            format!(
                "You need to run with root privileges.\nE.g. sudo {}",
                args.iter().fold(String::new(), |a, b| a + b + " ").trim()
            )
            .as_str(),
        );
    }

    args.iter().skip(1).for_each(|arg| {
        if std::fs::metadata(arg.clone()).is_ok()
            && std::fs::metadata(arg.clone()).unwrap().is_dir()
        {
            root = arg.to_string();
        }
    });
    if args.contains(&"--help".to_string()) {
        println!("{HELP}");
        std::process::exit(0);
    }
    if !root.is_empty() {
        let fstab = match genfstab::gen_from_mounts(root.as_str(), args.contains(&"-U".to_string()))
        {
            Ok(fstab) => fstab,
            Err(error) => panic!("{:?}", error),
        };

        if args.contains(&"--to-fstab".to_string()) {
            let etc_path = root_etc_path(root.as_str());
            if !std::path::Path::new(&etc_path).exists() {
                error(
                    format!("{etc_path} does not exists.\nPlease remove --to-fstab option.")
                        .as_str(),
                );
            } else if !std::fs::metadata(etc_path.clone()).unwrap().is_dir() {
                error(
                    format!("{etc_path} is not a directory.\nPlease remove --to-fstab option.")
                        .as_str(),
                );
            } else {
                let fstab_path = format!("{etc_path}/fstab");
                if std::fs::metadata(fstab_path.clone()).is_ok() {
                    if ask_confirm(
                        format!("{fstab_path} already exists.\nRewrite? (y/N): ").as_str(),
                        false,
                    ) {
                        write_to_file(fstab_path, fstab);
                    } else {
                        error("Fstab was not written.")
                    }
                } else {
                    write_to_file(fstab_path, fstab);
                }
            }
        } else {
            fstab.iter().for_each(|l| println!("{l}\n"))
        }
    } else {
        println!("DIRECTORY NOT FOUND.");
        std::process::exit(1);
    }
}
