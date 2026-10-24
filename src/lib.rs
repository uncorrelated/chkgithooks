enum Position {
    Table,
    Name,
    Key,
    Value,
}

#[derive(Debug)]
pub struct Record {
    pub table: String,
    pub name: String,
    pub key: String,
    pub value: String,
}

impl Record {
    #[allow(dead_code)]
    fn new(table: String, name: String, key: String, value: String) -> Self {
        Record {
            table,
            name,
            key,
            value,
        }
    }
    pub fn from(string: &str) -> Self {
        let chars: Vec<char> = string.chars().collect();

        let mut stack_table: Vec<char> = Vec::new();
        let mut stack_name: Vec<char> = Vec::new();
        let mut stack_key: Vec<char> = Vec::new();
        let mut stack_value: Vec<char> = Vec::new();

        let mut mode: Position = Position::Key;
        let mut trim_ws: bool = true;
        let mut in_quote: bool = false;
        let mut in_escape: bool = false;

        for c in chars.iter() {
            if (!in_quote && !in_escape) && ('#' == *c || ';' == *c) {
                break;
            }
            match mode {
                Position::Table => {
                    if ']' == *c {
                        mode = Position::Key;
                    } else if trim_ws {
                        if !c.is_whitespace() {
                            stack_table.push(c.to_ascii_lowercase());
                            trim_ws = false;
                        }
                    } else {
                        if c.is_whitespace() || '.' == *c {
                            in_quote = false;
                            in_escape = false;
                            mode = Position::Name;
                            continue;
                        } else if '"' == *c {
                            in_quote = true;
                            in_escape = false;
                            mode = Position::Name;
                            continue;
                        }
                        stack_table.push(c.to_ascii_lowercase());
                    }
                }
                Position::Name => {
                    if in_escape {
                        stack_name.push(*c);
                        in_escape = false;
                        continue;
                    }
                    if '\\' == *c {
                        in_escape = true;
                        continue;
                    }
                    if in_quote {
                        if '"' == *c {
                            in_quote = false;
                            continue;
                        }
                    } else {
                        if ']' == *c {
                            mode = Position::Key;
                            continue;
                        } else if '"' == *c {
                            in_quote = true;
                            continue;
                        }
                    }
                    stack_name.push(*c);
                }
                Position::Key => {
                    if '=' == *c {
                        mode = Position::Value;
                        trim_ws = true;
                    } else if '[' == *c {
                        mode = Position::Table;
                        stack_table = Vec::new();
                        stack_name = Vec::new();
                        stack_key = Vec::new();
                        trim_ws = true;
                    } else if !c.is_whitespace() {
                        stack_key.push(c.to_ascii_lowercase());
                    }
                }
                Position::Value => {
                    if trim_ws && c.is_whitespace() {
                        continue;
                    }
                    trim_ws = false;
                    if in_escape {
                        stack_value.push(*c);
                        in_escape = false;
                        continue;
                    }
                    if '\\' == *c {
                        in_escape = true;
                        continue;
                    }
                    if in_quote {
                        if '"' == *c {
                            in_quote = false;
                            continue;
                        }
                    } else {
                        if '"' == *c {
                            in_quote = true;
                            continue;
                        }
                    }
                    stack_value.push(*c);
                }
            };
        }

        let mut len = stack_value.len();
        while 0 < len {
            if !stack_value[len - 1].is_whitespace() {
                break;
            }
            len -= 1;
        }
        if 0 < len {
            stack_value = stack_value[0..len].to_vec();
        } else {
            stack_value = Vec::new();
        }

        Record {
            table: String::from_iter(stack_table),
            name: String::from_iter(stack_name),
            key: String::from_iter(stack_key),
            value: String::from_iter(stack_value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(t: &str, n: &str, k: &str, v: &str, r: &Record) {
        chk_table(t, r);
        chk_name(n, r);
        chk_key(k, r);
        chk_value(v, r);
    }
    fn chk_table(t: &str, r: &Record) {
        assert!(t == r.table, "table: '{}'!='{}'\n", t, r.table);
    }
    fn chk_name(n: &str, r: &Record) {
        assert!(n == r.name, "name: '{}'!='{}'\n", n, r.name);
    }
    fn chk_key(k: &str, r: &Record) {
        assert!(k == r.key, "key: '{}'!='{}'\n", k, r.key);
    }
    fn chk_value(v: &str, r: &Record) {
        assert!(v == r.value, "value: '{}'!='{}'\n", v, r.value);
    }
    #[test]
    fn test_fullset() {
        let string = String::from("[table] key = value");
        let record = Record::from(&string);
        check("table", "", "key", "value", &record);
    }
    #[test]
    fn test_just_table() {
        let string = String::from("[table]");
        let record = Record::from(&string);
        check("table", "", "", "", &record);
    }
    #[test]
    fn test_key_value() {
        let string = String::from("       key =     a a a # example");
        let record = Record::from(&string);
        check("", "", "key", "a a a", &record);
    }
    #[test]
    fn test_uppercase() {
        let string = String::from(" [TABLE] [TABLE2] KEY = VALUE");
        let record = Record::from(&string);
        check("table2", "", "key", "VALUE", &record);
    }
    #[test]
    fn test_with_name() {
        let string = String::from(r#"[hook "hk-pre-commit"]"#);
        let record = Record::from(&string);
        check("hook", "hk-pre-commit", "", "", &record);
    }
    #[test]
    fn test_dot_separator() {
        let string = String::from(r#"[hook.hk-pre-commit]"#);
        let record = Record::from(&string);
        check("hook", "hk-pre-commit", "", "", &record);
    }
    #[test]
    fn test_quote_separator() {
        let string = String::from(r#"[hook"hk-pre-commit"]"#);
        let record = Record::from(&string);
        check("hook", "hk-pre-commit", "", "", &record);
    }
    #[test]
    fn test_sandwitch() {
        let string = String::from(r#"[hook "evil"] command = /tmp/malware.sh [user2]"#);
        let record = Record::from(&string);
        check(
            "hook",
            "evil",
            "command",
            "/tmp/malware.sh [user2]",
            &record,
        );
    }
    #[test]
    fn test_semicolon() {
        let string = String::from(r#"[core ";"] sshCommand = /tmp/malware.sh"#);
        let record = Record::from(&string);
        check("core", ";", "sshcommand", "/tmp/malware.sh", &record);
    }
    #[test]
    fn test_sharp_in_quote() {
        let string = String::from(r#"[alias]     st = "!# \n /tmp/malware.sh""#);
        let record = Record::from(&string);
        check("alias", "", "st", r#"!# n /tmp/malware.sh"#, &record);
    }
}
