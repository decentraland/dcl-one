//! A console line sent from outside the process (the web page's console RPC, the native loopback
//! server, the CEF HUD's page), split into the command's name and its arguments.
//!
//! Words are split on whitespace, and a double-quoted run is part of a word with its quotes
//! removed, so `/chat nearby "two  words"` gives the message `two  words`. Inside the quotes `\"`
//! and `\\` escape; a backslash anywhere else is kept, so a Windows path needs no quoting, and a
//! single quote is a character like any other, so `it's` needs none either. A command whose
//! last argument takes the rest of the line (`trailing_var_arg`, how the JSON commands take their
//! JSON) gets its arguments split on whitespace alone, quotes kept, so the JSON it joins back
//! together is the JSON sent.

/// `/name args` (the `/` optional) -> the command's name with its `/`, and the line after it.
pub fn split_name(line: &str) -> Option<(String, &str)> {
    let line = line.trim_start();
    let end = line.find(char::is_whitespace).unwrap_or(line.len());
    let (name, rest) = line.split_at(end);
    if name.is_empty() {
        return None;
    }
    let name = if name.starts_with('/') {
        name.to_owned()
    } else {
        format!("/{name}")
    };
    Some((name, rest))
}

/// Whether a command's last argument takes the rest of the line.
pub fn takes_rest(command: &clap::Command) -> bool {
    command
        .get_arguments()
        .any(clap::Arg::is_trailing_var_arg_set)
}

const UNCLOSED: &str = "a double quote is not closed";

/// The arguments in the line after a command's name: as words with quotes, or on whitespace alone
/// when the command takes the rest of the line. An unclosed quote is an error.
pub fn split_arguments(rest: &str, takes_rest: bool) -> Result<Vec<String>, String> {
    if takes_rest {
        return Ok(rest.split_whitespace().map(String::from).collect());
    }
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => words.extend(word.take()),
            '"' => {
                let word = word.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        None => return Err(UNCLOSED.to_owned()),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(escaped @ ('"' | '\\')) => word.push(escaped),
                            Some(other) => {
                                word.push('\\');
                                word.push(other);
                            }
                            None => return Err(UNCLOSED.to_owned()),
                        },
                        Some(other) => word.push(other),
                    }
                }
            }
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    words.extend(word);
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(rest: &str) -> Vec<String> {
        split_arguments(rest, false).unwrap()
    }

    #[test]
    fn the_name_gets_its_slash_and_the_rest_follows_it() {
        assert_eq!(
            split_name("  teleport 10   20 "),
            Some(("/teleport".to_owned(), " 10   20 "))
        );
        assert_eq!(
            split_name("/world_state"),
            Some(("/world_state".to_owned(), ""))
        );
        assert_eq!(split_name("   "), None);
    }

    #[test]
    fn words_split_on_whitespace() {
        assert_eq!(words(" 10   20 "), ["10", "20"]);
        assert!(words("   ").is_empty());
    }

    #[test]
    fn a_quoted_run_is_one_word_without_its_quotes() {
        assert_eq!(words(r#"nearby "two  words""#), ["nearby", "two  words"]);
        assert_eq!(words("nearby it's 'fine'"), ["nearby", "it's", "'fine'"]);
        assert_eq!(words(r#"--text="a b"c"#), ["--text=a bc"]);
        assert_eq!(words(r#""""#), [""]);
    }

    #[test]
    fn quotes_escape_a_quote_and_a_backslash_only() {
        assert_eq!(words(r#""say \"hi\" \\ \n""#), [r#"say "hi" \ \n"#]);
    }

    #[test]
    fn backslashes_and_hashes_outside_quotes_are_kept() {
        assert_eq!(words(r"C:\scenes\a #tag"), [r"C:\scenes\a", "#tag"]);
    }

    #[test]
    fn an_unclosed_quote_is_refused() {
        assert_eq!(
            split_arguments(r#"nearby "two words"#, false),
            Err(UNCLOSED.to_owned())
        );
        assert_eq!(
            split_arguments(r#""ends in \""#, false),
            Err(UNCLOSED.to_owned())
        );
    }

    #[test]
    fn a_command_taking_the_rest_of_the_line_keeps_its_json_as_sent() {
        assert_eq!(
            split_arguments(r#" 512 {"text":"a b"} "#, true).unwrap(),
            ["512", r#"{"text":"a"#, r#"b"}"#]
        );
    }
}
