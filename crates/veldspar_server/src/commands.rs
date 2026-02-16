#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Noop,
    Stop,
    List,
    Say(String),
    TimeSet(f32),
    Kick(String),
    Teleport {
        player: String,
        x: f32,
        y: f32,
        z: f32,
    },
    Help,
    InvalidUsage(String),
    Unknown(String),
}

pub fn parse_command(line: &str) -> Command {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Command::Noop;
    }

    let input = trimmed.strip_prefix('/').unwrap_or(trimmed);
    if input.is_empty() {
        return Command::Noop;
    }

    let mut head_tail = input.splitn(2, char::is_whitespace);
    let command = head_tail.next().unwrap_or_default().to_ascii_lowercase();
    let rest = head_tail.next().unwrap_or("").trim();

    match command.as_str() {
        "stop" => Command::Stop,
        "list" => Command::List,
        "say" => {
            if rest.is_empty() {
                Command::InvalidUsage("Usage: /say <message>".to_string())
            } else {
                Command::Say(rest.to_string())
            }
        }
        "time" => {
            let mut args = rest.split_whitespace();
            match (args.next(), args.next(), args.next()) {
                (Some(mode), Some(value), None) if mode.eq_ignore_ascii_case("set") => {
                    match value.parse::<f32>() {
                        Ok(parsed) if (0.0..=1.0).contains(&parsed) => Command::TimeSet(parsed),
                        _ => Command::InvalidUsage(
                            "Usage: /time set <value>, where value is between 0.0 and 1.0"
                                .to_string(),
                        ),
                    }
                }
                _ => Command::InvalidUsage(
                    "Usage: /time set <value>, where value is between 0.0 and 1.0".to_string(),
                ),
            }
        }
        "kick" => {
            if rest.is_empty() {
                Command::InvalidUsage("Usage: /kick <player|id>".to_string())
            } else {
                Command::Kick(rest.to_string())
            }
        }
        "tp" => {
            let mut args = rest.split_whitespace();
            match (
                args.next(),
                args.next(),
                args.next(),
                args.next(),
                args.next(),
            ) {
                (Some(player), Some(x), Some(y), Some(z), None) => {
                    let parsed = (x.parse::<f32>(), y.parse::<f32>(), z.parse::<f32>());
                    match parsed {
                        (Ok(x), Ok(y), Ok(z)) => Command::Teleport {
                            player: player.to_string(),
                            x,
                            y,
                            z,
                        },
                        _ => Command::InvalidUsage(
                            "Usage: /tp <player|id> <x> <y> <z>".to_string(),
                        ),
                    }
                }
                _ => Command::InvalidUsage("Usage: /tp <player|id> <x> <y> <z>".to_string()),
            }
        }
        "help" => Command::Help,
        _ => Command::Unknown(input.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_command};

    #[test]
    fn parses_required_commands() {
        assert_eq!(parse_command("/stop"), Command::Stop);
        assert_eq!(parse_command("list"), Command::List);
        assert_eq!(parse_command("/help"), Command::Help);
        assert_eq!(
            parse_command("/say hello world"),
            Command::Say("hello world".to_string())
        );
        assert_eq!(parse_command("/time set 0.25"), Command::TimeSet(0.25));
        assert_eq!(parse_command("/kick Player2"), Command::Kick("Player2".into()));
    }

    #[test]
    fn parses_tp_and_reports_usage_errors() {
        assert_eq!(
            parse_command("/tp Player1 10 64 -2"),
            Command::Teleport {
                player: "Player1".to_string(),
                x: 10.0,
                y: 64.0,
                z: -2.0
            }
        );
        assert_eq!(
            parse_command("/time set 2.0"),
            Command::InvalidUsage(
                "Usage: /time set <value>, where value is between 0.0 and 1.0".to_string()
            )
        );
        assert_eq!(
            parse_command("/kick"),
            Command::InvalidUsage("Usage: /kick <player|id>".to_string())
        );
    }
}
