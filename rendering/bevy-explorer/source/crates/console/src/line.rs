//! A console line, split into the command's name and its arguments. Every way a line reaches
//! the console splits it here: the chat input, the web page's console RPC, the native loopback
//! server and the CEF HUD's page.
//!
//! Words are split on whitespace, and a double-quoted run is part of a word with its quotes
//! removed, so `/chat nearby "two  words"` gives the message `two  words`. Inside the quotes `\"`
//! and `\\` escape; a backslash anywhere else is kept, so a Windows path needs no quoting, and a
//! single quote is a character like any other, so `it's` needs none either. A command whose
//! last argument takes the rest of the line (`trailing_var_arg`, how the JSON commands take their
//! JSON) gets its arguments split on whitespace alone, quotes kept: the command joins them with
//! one space, so its JSON arrives as sent except that a run of whitespace inside one of its
//! strings becomes a single space.

use bevy_console::ConsoleConfiguration;

/// A line as the command's name, with its `/`, and its arguments. An empty line and an unclosed
/// quote are errors; a name the console does not know is split as words.
pub fn split(line: &str, config: &ConsoleConfiguration) -> Result<(String, Vec<String>), String> {
    let (name, rest) = split_name(line).ok_or_else(|| "empty command".to_owned())?;
    let takes_rest = config.commands.get(name.as_str()).is_some_and(takes_rest);
    let arguments = split_arguments(rest, takes_rest).map_err(|e| format!("{name}: {e}"))?;
    Ok((name, arguments))
}

/// `/name args` (the `/` optional) -> the command's name with its `/`, and the line after it.
fn split_name(line: &str) -> Option<(String, &str)> {
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
fn takes_rest(command: &clap::Command) -> bool {
    command
        .get_arguments()
        .any(clap::Arg::is_trailing_var_arg_set)
}

const UNCLOSED: &str = "a double quote is not closed";

/// The arguments in the line after a command's name: as words with quotes, or on whitespace alone
/// when the command takes the rest of the line. An unclosed quote is an error.
fn split_arguments(rest: &str, takes_rest: bool) -> Result<Vec<String>, String> {
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
    fn a_command_taking_the_rest_of_the_line_keeps_its_quotes() {
        assert_eq!(
            split_arguments(r#" 512 {"text":"a  b"} "#, true).unwrap(),
            ["512", r#"{"text":"a"#, r#"b"}"#]
        );
    }

    #[derive(clap::Parser)]
    struct Words {
        _channel: String,
        _message: String,
    }

    #[derive(clap::Parser)]
    struct Rest {
        _entity: u32,
        #[arg(trailing_var_arg = true, num_args = 1..)]
        _json: Vec<String>,
    }

    fn config() -> ConsoleConfiguration {
        use clap::CommandFactory;
        let mut config = ConsoleConfiguration::default();
        config.commands.insert("/chat", Words::command());
        config.commands.insert("/set", Rest::command());
        config
    }

    #[test]
    fn a_line_splits_by_what_its_command_takes() {
        let config = config();
        assert_eq!(
            split(r#"chat nearby "two words""#, &config),
            Ok((
                "/chat".to_owned(),
                vec!["nearby".into(), "two words".into()]
            ))
        );
        assert_eq!(
            split(r#"/set 512 {"text":"a b"}"#, &config),
            Ok((
                "/set".to_owned(),
                vec!["512".into(), r#"{"text":"a"#.into(), r#"b"}"#.into()]
            ))
        );
        assert_eq!(
            split(r#"/unknown "a b""#, &config),
            Ok(("/unknown".to_owned(), vec!["a b".into()]))
        );
    }

    #[test]
    fn an_empty_line_and_an_unclosed_quote_name_the_fault() {
        let config = config();
        assert_eq!(split("   ", &config), Err("empty command".to_owned()));
        assert_eq!(
            split(r#"/chat nearby "two"#, &config),
            Err(format!("/chat: {UNCLOSED}"))
        );
    }
}
