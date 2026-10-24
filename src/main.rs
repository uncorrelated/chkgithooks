// Written by zombie, uncorrelated
// MIT License
use chkgithooks::Record;
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
    let mut nhooks: i32 = 0;
    let mut _nfiles: i32 = 0;

    // グローバル設定でhooksPathが無ければ終了
    // hooksPath = /dev/null などがあれば、.git/hooks/ に悪意のあるスクリプトがあっても起動しないので無問題
    if let Some(home_dir) = dirs::home_dir() {
        let global_config_path = home_dir.join(".gitconfig");
        if global_config_path.exists() {
            let (nhooks, _nfiles) = chkconfig(&global_config_path, false, 0);
            if 0 == nhooks {
                eprint!(
                    "core.hooksPath in {} is needed to run git safely.
You can add it by 'git config --add --global core.hooksPath /dev/null'\n",
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
                (nhooks, _nfiles) = chkconfig(&config_path, true, 0);
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

fn chkconfig(file: &Path, is_local: bool, mut nfiles: i32) -> (i32, i32) {
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

    // Line continuationのためのバッファー
    let mut buffers: Vec<String> = Vec::new();

    if let Ok(handle) = File::open(file) {
        let filename = file.to_str().unwrap();
        let parent_dir = file.parent().unwrap_or(Path::new("."));
        let reader = BufReader::new(handle);
        let mut lines = reader.lines().peekable();
        let mut table = String::from("");
        'reading_loop: while let Some(line) = lines.next() {
            let mut line = line.expect("Couldn't read the file.");

            // Line continuationの処理
            if !lines.peek().is_none() {
                if let Some(stripped) = line.strip_suffix('\\') {
                    buffers.push(stripped.to_string());
                    continue;
                }
            }
            if 0 < buffers.len() {
                line = format!("{}{}", buffers.concat(), line);
                buffers = Vec::new();
            }

            let record = Record::from(&line);
            // eprint!("{:?}\n", record);
            if "" != record.table {
                table = record.table.clone();
            }
            if "hook" == record.table && is_local {
                eprintln!("hook: \"{}\" {}", filename, line);
                nhooks = nhooks + 1;
            } else {
                if "core" == table {
                    if is_local {
                        if "fsmonitor" == record.key && "true" != record.value {
                            eprintln!("fsmonitor: \"{}\" {}", filename, line);
                            nhooks += 1;
                            continue;
                        }
                        let key = [
                            "hookspath",
                            "pager",
                            "editor",
                            "sshcommand",
                            "gitproxy",
                            "askpass",
                        ];
                        for k in key.iter() {
                            if *k == record.key {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "hookspath" == record.key {
                        nhooks += 1;
                        continue;
                    }
                } else if "path" == record.key {
                    let next_path = parent_dir.join(record.value);
                    if is_local {
                        eprintln!("path: \"{}\"  \"{}\"", filename, next_path.display());
                    }
                    let (n, m) = chkconfig(&next_path, is_local, nfiles);
                    nhooks += n;
                    nfiles = m;
                } else if is_local {
                    if "diff" == table {
                        if "external" == record.key {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue;
                        }
                        let key_suffix = [".command", ".textconv"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "filter" == table {
                        let key_suffix = [".clean", ".smudge"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "merge" == table {
                        if record.key.ends_with(".driver") {
                                eprintln!("{}: \"{}\" {}", record.key, filename, line);
                           nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "difftool" == table {
                        let key_suffix = [".cmd", ".path"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "mergetool" == table {
                        let key_suffix = [".cmd", ".path"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "credential" == table {
                        if "helper" == record.key && record.value.starts_with('!') {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "remote" == table {
                        let key_suffix = [".uploadpack", ".receivepack", "proxyauthmethod"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "gpg" == table {
                        let key_suffix = ["program", ".program", "ssh.defaultkeycommand"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "alias" == table {
                        if record.value.starts_with('!') {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "sendmail" == table {
                        let key = ["sendmailcmd", "smtpserver", "validate"];
                        for k in key.iter() {
                            if *k == record.key {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "man" == table {
                        let key_suffix = ["cmd", "path"];
                        for k in key_suffix.iter() {
                            if record.key.ends_with(k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "sequence" == table {
                        if "editor" == record.key {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    }
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
