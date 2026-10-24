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
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "hookspath" == record.key {
                        nhooks += 1;
                        continue;
                    }
                } else if chkkey(&record.key, "path") {
                    let next_path = parent_dir.join(record.value);
                    if is_local {
                        eprintln!("path: \"{}\"  \"{}\"", filename, next_path.display());
                    }
                    let (n, m) = chkconfig(&next_path, is_local, nfiles);
                    nhooks += n;
                    nfiles = m;
                } else if is_local {
                    if "diff" == table {
                        if chkkey(&record.key, "external") {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue;
                        }
                        let key_suffix = ["command", "textconv"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "filter" == table {
                        let key_suffix = ["clean", "smudge"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "merge" == table {
                        if chkkey(&record.key, "driver") {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "difftool" == table {
                        let key_suffix = ["cmd", "path"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "mergetool" == table {
                        let key_suffix = ["cmd", "path"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "credential" == table {
                        if chkkey(&record.key, "helper") {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "remote" == table {
                        let key_suffix = ["uploadpack", "receivepack", "proxyauthmethod"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "gpg" == table {
                        let key_suffix = ["program", "program", "defaultkeycommand"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "alias" == table {
                        if record.value.contains('!') {
                            eprintln!("{}: \"{}\" {}", record.key, filename, line);
                            nhooks += 1;
                            continue 'reading_loop;
                        }
                    } else if "sendmail" == table {
                        let key = ["sendmailcmd", "smtpserver", "validate"];
                        for k in key.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "man" == table {
                        let key_suffix = ["cmd", "path"];
                        for k in key_suffix.iter() {
                            if chkkey(&record.key, k) {
                                eprintln!("{}: \"{}\" {}", k, filename, line);
                                nhooks += 1;
                                continue 'reading_loop;
                            }
                        }
                    } else if "sequence" == table {
                        if chkkey(&record.key, "editor") {
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

fn chkkey(key: &str, condition: &str) -> bool {
    let key_a = key.as_bytes();
    let key_l = key_a.len();
    let cnd_a = condition.as_bytes();
    let cnd_l = cnd_a.len();

    if 0 == key_l {
        if 0 == cnd_l {
            return true;
        }
        return false;
    }

    if key_l < cnd_l {
        return false;
    }

    let mut n = key_l;
    let mut m = cnd_l;

    loop {
        n -= 1;
        m -= 1;
        if key_a[n] != cnd_a[m] {
            return false;
        }
        if 0 == m {
            break;
        }
    }

    // .${condition} の場合をチェック
    if key_l > cnd_l && 0x2e != key_a[n - 1] {
        return false;
    }

    return true;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_chkkey_same() {
        let a = "abc";
        let b = "abc";
        assert!(true == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
    #[test]
    fn test_chkkey_different() {
        let a = "abc";
        let b = "adc";
        assert!(false == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
    #[test]
    fn test_chkkey_suffix() {
        let a = "efg.abc";
        let b = "abc";
        assert!(true == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
    #[test]
    fn test_chkkey_suffix_different() {
        let a = "efg.abc";
        let b = "adc";
        assert!(false == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
    #[test]
    fn test_chkkey_zero_length() {
        let a = "";
        let b = "";
        assert!(true == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
    #[test]
    fn test_chkkey_zero_length_different() {
        let a = "";
        let b = "abc";
        assert!(false == chkkey(a, b), "'{}' <> '{}'\n", a, b);
    }
}
