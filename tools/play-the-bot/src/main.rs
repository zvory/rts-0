use std::{
    process::ExitCode,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tungstenite::{connect, Message, WebSocket};
use url::Url;

const BETA_SERVER: &str = "https://rts-0-zvorygin-beta.fly.dev/";
const BOT_PROFILE_ID: &str = "jeffs_ai";
const BOT_DISPLAY_NAME: &str = "Jeff's AI";
const HOST_DISPLAY_NAME: &str = "Bot Lobby Host";

#[derive(Debug, Eq, PartialEq)]
struct Options {
    server: String,
    room: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct LobbyDecision {
    bot_present: bool,
    human_opponent_present: bool,
    human_opponent_ready: bool,
    can_start: bool,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_options(&args).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("play_the_bot error: {error}");
            eprintln!("Press Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
            ExitCode::FAILURE
        }
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut server = BETA_SERVER.to_string();
    let mut room = default_room_name()?;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--server" | "--room" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                match flag {
                    "--server" => server = resolve_server(value),
                    "--room" => room = value.clone(),
                    _ => unreachable!(),
                }
            }
            "--help" | "-h" => return Err(usage()),
            _ => return Err(format!("unknown option: {flag}\n{}", usage())),
        }
        index += 1;
    }
    validate_lobby_text("room", &room, 48)?;
    Ok(Options { server, room })
}

fn usage() -> String {
    "usage: play_the_bot [--room <name>] [--server beta|URL]".to_string()
}

fn default_room_name() -> Result<String, String> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_secs()
        % 1_000_000;
    Ok(format!("Play The Bot {suffix:06}"))
}

fn validate_lobby_text(label: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.len() > max_len || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} must be at most {max_len} characters with no controls"
        ));
    }
    Ok(())
}

fn resolve_server(value: &str) -> String {
    match value {
        "beta" => BETA_SERVER.to_string(),
        other => other.to_string(),
    }
}

fn normalize_server_url(value: &str) -> Result<Url, String> {
    let mut url = Url::parse(value).map_err(|error| format!("invalid server URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("server URL must use http or https".to_string());
    }
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn websocket_url(server: &Url) -> Result<Url, String> {
    let mut url = server.clone();
    let scheme = match server.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => return Err("server URL must use http or https".to_string()),
    };
    url.set_scheme(scheme)
        .map_err(|()| "could not construct WebSocket URL".to_string())?;
    url.set_path("/ws");
    Ok(url)
}

fn create_lobby(server: &Url, room: &str) -> Result<(), String> {
    let endpoint = server
        .join("api/lobbies")
        .map_err(|error| format!("invalid lobby endpoint: {error}"))?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("could not build HTTP client: {error}"))?
        .post(endpoint)
        .json(&serde_json::json!({ "room": room }))
        .send()
        .map_err(|error| format!("lobby creation request failed: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "lobby creation failed with HTTP {}",
            response.status()
        ))
    }
}

type Socket = WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn run(options: Options) -> Result<(), String> {
    let server = normalize_server_url(&options.server)?;
    create_lobby(&server, &options.room)?;
    let ws_url = websocket_url(&server)?;
    let (mut socket, _) = connect(ws_url.as_str())
        .map_err(|error| format!("WebSocket connection failed: {error}"))?;

    send_json(
        &mut socket,
        serde_json::json!({
            "t": "join",
            "name": HOST_DISPLAY_NAME,
            "room": options.room,
            "spectator": true
        }),
    )?;

    let host_id = wait_for_lobby_join(&mut socket, &options.room)?;
    send_json(
        &mut socket,
        serde_json::json!({
            "t": "addAi",
            "teamId": 1,
            "aiProfileId": BOT_PROFILE_ID
        }),
    )?;

    println!("play_the_bot is ready");
    println!("Server: {}", server.as_str());
    println!("Room: {}", options.room);
    println!("Bot: {BOT_DISPLAY_NAME}");
    println!("Join this room as a player and click Ready.");

    let mut start_requested = false;
    loop {
        let value = read_json(&mut socket)?;
        reject_server_error(&value)?;
        match value.get("t").and_then(serde_json::Value::as_str) {
            Some("lobby") => {
                let decision = evaluate_lobby(&value, host_id);
                if !decision.bot_present {
                    return Err(format!("server did not add {BOT_DISPLAY_NAME}"));
                }
                if decision.human_opponent_present && !decision.human_opponent_ready {
                    println!("Player joined; waiting for Ready...");
                }
                if decision.human_opponent_ready && decision.can_start && !start_requested {
                    send_json(&mut socket, serde_json::json!({ "t": "start" }))?;
                    start_requested = true;
                    println!("Player is ready; starting immediately.");
                }
            }
            Some("start") => println!("Match started: player versus {BOT_DISPLAY_NAME}."),
            Some("observationReady") => {
                if let Some(run_id) = value.get("matchRunId").and_then(serde_json::Value::as_str) {
                    println!("Replay observation id: {run_id}");
                }
            }
            Some("gameOver") => {
                println!("Match complete.");
                println!("You may close this window.");
                return Ok(());
            }
            _ => {}
        }
    }
}

fn evaluate_lobby(value: &serde_json::Value, host_id: u64) -> LobbyDecision {
    let mut decision = LobbyDecision {
        can_start: value
            .get("canStart")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        ..LobbyDecision::default()
    };
    let players = value
        .get("players")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    for player in players {
        let id = player.get("id").and_then(serde_json::Value::as_u64);
        let is_ai = player
            .get("isAi")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let spectator = player
            .get("isSpectator")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if is_ai
            && player
                .get("aiProfileId")
                .and_then(serde_json::Value::as_str)
                == Some(BOT_PROFILE_ID)
        {
            decision.bot_present = true;
        } else if !is_ai && !spectator && id != Some(host_id) {
            decision.human_opponent_present = true;
            decision.human_opponent_ready |= player
                .get("ready")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
        }
    }
    decision
}

fn send_json(socket: &mut Socket, value: serde_json::Value) -> Result<(), String> {
    socket
        .send(Message::Text(value.to_string().into()))
        .map_err(|error| format!("could not send message: {error}"))
}

fn read_json(socket: &mut Socket) -> Result<serde_json::Value, String> {
    loop {
        match socket
            .read()
            .map_err(|error| format!("WebSocket read failed: {error}"))?
        {
            Message::Text(text) => {
                return serde_json::from_str(&text)
                    .map_err(|error| format!("server returned invalid JSON: {error}"))
            }
            Message::Ping(payload) => socket
                .send(Message::Pong(payload))
                .map_err(|error| format!("could not answer server ping: {error}"))?,
            Message::Close(frame) => {
                return Err(format!("server closed the connection: {frame:?}"))
            }
            _ => {}
        }
    }
}

fn wait_for_lobby_join(socket: &mut Socket, expected_room: &str) -> Result<u64, String> {
    let mut player_id = None;
    loop {
        let value = read_json(socket)?;
        reject_server_error(&value)?;
        match value.get("t").and_then(serde_json::Value::as_str) {
            Some("welcome") => {
                player_id = value.get("playerId").and_then(serde_json::Value::as_u64)
            }
            Some("lobby") => {
                if value.get("room").and_then(serde_json::Value::as_str) != Some(expected_room) {
                    return Err("server joined an unexpected room".to_string());
                }
                if let Some(id) = player_id {
                    return Ok(id);
                }
            }
            _ => {}
        }
    }
}

fn reject_server_error(value: &serde_json::Value) -> Result<(), String> {
    if value.get("t").and_then(serde_json::Value::as_str) == Some("error") {
        return Err(value
            .get("msg")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("server rejected the request")
            .to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_room_and_server_options() {
        let options = parse_options(&[
            "--room".into(),
            "Challenge".into(),
            "--server".into(),
            "beta".into(),
        ])
        .unwrap();
        assert_eq!(options.room, "Challenge");
        assert_eq!(options.server, BETA_SERVER);
    }

    #[test]
    fn starts_only_for_a_ready_human_with_jeffs_ai_present() {
        let lobby = serde_json::json!({
            "t": "lobby",
            "canStart": true,
            "players": [
                { "id": 1, "isAi": false, "isSpectator": true, "ready": false },
                { "id": 2, "isAi": true, "isSpectator": false, "ready": true, "aiProfileId": "jeffs_ai" },
                { "id": 3, "isAi": false, "isSpectator": false, "ready": true }
            ]
        });
        assert_eq!(
            evaluate_lobby(&lobby, 1),
            LobbyDecision {
                bot_present: true,
                human_opponent_present: true,
                human_opponent_ready: true,
                can_start: true,
            }
        );
    }

    #[test]
    fn does_not_treat_ai_2_1_as_our_bot() {
        let lobby = serde_json::json!({
            "canStart": true,
            "players": [
                { "id": 2, "isAi": true, "aiProfileId": "ai_2_1" },
                { "id": 3, "isAi": false, "isSpectator": false, "ready": true }
            ]
        });
        assert!(!evaluate_lobby(&lobby, 1).bot_present);
    }
}
