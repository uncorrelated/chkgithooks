// Written by zombie, uncorrelated
// MIT License
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

const CONFIG_FILENAME: &str = ".git/config";
const MAX_COUNT_FILES: i32 = 100;
const MAX_COUNT_HOOKS: i32 = 100;

fn main() {
    let re_section_name = Regex::new(r"^([ \t]*\[[^\]]*\])+").unwrap();
    let re_hooks_path = Regex::new(r"(?i)^[ \t]*hooksPath[ \t]*=").unwrap();
    let re_hook = Regex::new(r#"(?i)^[^#;]*\[[ \t]*["']?hook([^a-z0-9\_\-][^\]]*)?\]"#).unwrap();
    let re_path = Regex::new(r"(?i)^[ \t]*path[ \t]*=[ \t]*([^#]+)[ \t]*").unwrap();
    let mut nhooks: i32 = 0;
    let mut _nfiles: i32 = 0;

    // グローバル設定でhooksPathが無ければ終了
    // hooksPath = /dev/null などがあれば、.git/hooks/ に悪意のあるスクリプトがあっても起動しないので無問題
    if let Some(home_dir) = dirs::home_dir() {
        let global_config_path = home_dir.join(".gitconfig");
        if global_config_path.exists() {
            let (nhooks, _nfiles) = chkconfig(
                &global_config_path,
                &re_section_name,
                &re_hooks_path,
                &re_hook,
                &re_path,
                false,
                0,
            );
            if 0 == nhooks {
                eprint!(
                    "hooksPath in {} is needed to run git safely.\n",
                    global_config_path.to_str().unwrap()
                );
                std::process::exit(1);
            }
        } else {
            eprint!(
                "{} on which hooksPath is written is needed to run git safely.\n",
                global_config_path.to_str().unwrap()
            );
            std::process::exit(1);
        }
    }

    // カレントディレクトリーからルートまで遡って .git/config を探し、hooksPathもしくはhooksの設定の行を数える
    if let Ok(current_dir) = env::current_dir() {
        for ancestor in current_dir.ancestors() {
            let config_path = ancestor.join(CONFIG_FILENAME);
            if config_path.exists() {
                (nhooks, _nfiles) = chkconfig(
                    &config_path,
                    &re_section_name,
                    &re_hooks_path,
                    &re_hook,
                    &re_path,
                    true,
                    0,
                );
                break;
            }
        }
    }

    // hooksPathもしくはhooksの設定が無ければ、コマンドライン引数をgitに渡して起動する
    let args: Vec<String> = env::args().collect();
    if 0 == nhooks {
        let c_args: Vec<String> = args[1..args.len()].to_vec();
        let cmd_path = Path::new(&args[0]);
        let git_path_string = if Some("git") == cmd_path.file_name().and_then(|n| n.to_str()) {
            // gitとして呼び出されていたら、環境変数を参照して、本物へのパスとする
            env::var("TRUE_GIT_PATH").unwrap_or(String::from("/usr/bin/git"))
        } else {
            // そうでない場合は循環参照はしないので、
            String::from("git")
        };
        let status = Command::new(git_path_string)
            .args(&c_args)
            .status()
            .expect("failed to execute process");
        std::process::exit(status.code().unwrap_or(1));
    } else {
        std::process::exit(1);
    }
}

fn chkconfig(
    file: &Path,
    re_section_name: &Regex,
    re_hooks_path: &Regex,
    re_hook: &Regex,
    re_path: &Regex,
    is_local: bool,
    mut nfiles: i32,
) -> (i32, i32) {
    let mut nhooks: i32 = 0;

    nfiles += 1;
    if MAX_COUNT_FILES <= nfiles {
        if is_local {
            // ローカル設定のときは1以上で動作条件外
            return (1, nfiles);
        } else {
            // グローバル設定のときは0で動作条件外
            return (0, nfiles);
        }
    }

    if let Ok(handle) = File::open(file) {
        let filename = file.to_str().unwrap();
        let parent_dir = file.parent().unwrap_or(Path::new("."));
        let reader = BufReader::new(handle);
        for line in reader.lines() {
            let line = line.expect("Couldn't read the file.");
            if re_hook.is_match(&line) && is_local {
                if is_local {
                    eprintln!("hook: \"{}\" {}", filename, line);
                }
                nhooks = nhooks + 1;
            } else {
                let line = re_section_name.replace(&line, "");
                if re_hooks_path.is_match(&line) {
                    if is_local {
                        eprintln!("hooksPath: \"{}\" {}", filename, line);
                    }
                    nhooks += 1;
                } else if let Some(caps) = re_path.captures(&line) {
                    let raw_path = caps.get(1).unwrap().as_str().trim().trim_matches('"');
                    let next_path = parent_dir.join(raw_path);
                    if is_local {
                        eprintln!("path: \"{}\"  \"{}\"", filename, next_path.display());
                    }
                    let (n, m) = chkconfig(
                        &next_path,
                        &re_section_name,
                        &re_hooks_path,
                        &re_hook,
                        &re_path,
                        is_local,
                        nfiles,
                    );
                    nhooks += n;
                    nfiles = m;
                }
            }
            if is_local {
                if MAX_COUNT_FILES <= nfiles || MAX_COUNT_HOOKS <= nhooks {
                    return (nhooks, nfiles);
                }
            } else {
                if MAX_COUNT_FILES <= nfiles || 1 <= nhooks {
                    return (nhooks, nfiles);
                }
            }
        }
    }

    (nhooks, nfiles)
}

