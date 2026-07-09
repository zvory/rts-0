use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitSink, SinkExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use rts_server::lobby::{self, ConnectionWriter};
use rts_server::protocol::{
    default_snapshot_codec, encode_snapshot_frame_with_diagnostics, ServerMessage, SnapshotFrame,
    SNAPSHOT_FRAME_KIND_TEXT,
};
use rts_sim::perf;

pub(crate) fn spawn(
    player_id: u32,
    mut sink: SplitSink<WebSocket, Message>,
    writer_rx: ConnectionWriter,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        run(player_id, &mut sink, writer_rx).await;
        let _ = sink.close().await;
    })
}

async fn run(
    player_id: u32,
    sink: &mut SplitSink<WebSocket, Message>,
    writer_rx: ConnectionWriter,
) {
    let (mut reliable_rx, snapshots, observer_analysis, mut writer_stats) = writer_rx.into_parts();
    let mut reliable_closed = false;

    'write_loop: loop {
        let snapshot_waiting = snapshots.has_pending();
        while !reliable_closed {
            match reliable_rx.try_recv() {
                Ok(msg) => {
                    writer_stats.note_reliable_for_snapshot(snapshot_waiting, &snapshots);
                    let command_receipt = matches!(msg, ServerMessage::CommandReceipt { .. });
                    let (keep_writing, _) = send_server_message(player_id, sink, msg).await;
                    if !keep_writing {
                        break 'write_loop;
                    }
                    writer_stats.record_reliable_sent(command_receipt);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    reliable_closed = true;
                    break;
                }
            }
        }

        if let Some(snapshot_send) = snapshots.take_for_send(&mut writer_stats) {
            let (snapshot, snapshot_stats) = snapshot_send.into_parts();
            let (keep_writing, writer_send_stats) =
                send_server_message(player_id, sink, ServerMessage::Snapshot(snapshot)).await;
            if !keep_writing {
                break 'write_loop;
            }
            if let Some(writer_send_stats) = writer_send_stats {
                writer_stats.record_snapshot_sent(snapshot_stats, writer_send_stats);
            }
            continue;
        }

        if let Some(payload) = observer_analysis.take() {
            let (keep_writing, _) =
                send_server_message(player_id, sink, ServerMessage::ObserverAnalysis(payload))
                    .await;
            if !keep_writing {
                break 'write_loop;
            }
            continue;
        }

        if reliable_closed {
            break;
        }

        tokio::select! {
            maybe_msg = reliable_rx.recv() => {
                match maybe_msg {
                    Some(msg) => {
                        writer_stats.note_reliable_message(snapshots.has_pending());
                        let command_receipt = matches!(msg, ServerMessage::CommandReceipt { .. });
                        let (keep_writing, _) = send_server_message(player_id, sink, msg).await;
                        if !keep_writing {
                            break 'write_loop;
                        }
                        writer_stats.record_reliable_sent(command_receipt);
                    }
                    None => reliable_closed = true,
                }
            }
            _ = snapshots.notified() => {}
            _ = observer_analysis.notified() => {}
        }
    }
}

async fn send_server_message(
    player_id: u32,
    sink: &mut SplitSink<WebSocket, Message>,
    msg: ServerMessage,
) -> (bool, Option<lobby::SnapshotWriterSendStats>) {
    let message_kind = match &msg {
        ServerMessage::Snapshot(_) => "snapshot",
        ServerMessage::Lobby { .. } => "lobby",
        ServerMessage::MatchCountdown { .. } => "match_countdown",
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::Start(_) => "start",
        ServerMessage::RoomTimeState(_) => "room_time_state",
        ServerMessage::LivePauseState(_) => "live_pause_state",
        ServerMessage::ObserverAnalysis(_) => "observer_analysis",
        ServerMessage::JoinReplayPrompt { .. } => "join_replay_prompt",
        ServerMessage::LabState(_) => "lab_state",
        ServerMessage::LabResult(_) => "lab_result",
        ServerMessage::ShutdownWarning { .. } => "shutdown_warning",
        ServerMessage::Error { .. } => "error",
        ServerMessage::GameOver { .. } => "game_over",
        ServerMessage::Pong { .. } => "pong",
        #[allow(unreachable_patterns)]
        _ => "other",
    };
    let serialize_start = Instant::now();
    let mut snapshot_codec = "";
    let mut snapshot_codec_version = 0;
    let (encoded, snapshot_payload) = match msg {
        ServerMessage::Snapshot(snapshot) => {
            let codec = default_snapshot_codec();
            snapshot_codec = codec.name();
            snapshot_codec_version = codec.version();
            match encode_snapshot_frame_with_diagnostics(&snapshot, codec) {
                Ok((frame, payload)) => {
                    (Ok(ServerFrame::from_snapshot_frame(frame)), Some(payload))
                }
                Err(err) => (Err(err), None),
            }
        }
        reliable => (
            serde_json::to_string(&reliable)
                .map(ServerFrame::Text)
                .map_err(rts_server::protocol::SnapshotEncodeError::from),
            None,
        ),
    };
    let serialize_duration = serialize_start.elapsed();
    match encoded {
        Ok(frame) => {
            let frame_kind = frame.frame_kind();
            let bytes = frame.len();
            let send_start = Instant::now();
            if sink.send(frame.into_message()).await.is_err() {
                let send_duration = send_start.elapsed();
                log_writer_message(
                    player_id,
                    message_kind,
                    snapshot_codec,
                    snapshot_codec_version,
                    frame_kind,
                    serialize_duration,
                    send_duration,
                    bytes,
                );
                return (false, None);
            }
            let send_duration = send_start.elapsed();
            log_writer_message(
                player_id,
                message_kind,
                snapshot_codec,
                snapshot_codec_version,
                frame_kind,
                serialize_duration,
                send_duration,
                bytes,
            );
            let snapshot_stats = snapshot_payload.map(|payload| lobby::SnapshotWriterSendStats {
                serialize_ms: saturating_duration_ms_u32(serialize_duration),
                send_ms: saturating_duration_ms_u32(send_duration),
                bytes: bytes.min(u32::MAX as usize) as u32,
                payload,
            });
            (true, snapshot_stats)
        }
        Err(err) => {
            rts_server::log_warn!(player_id, %err, "failed to serialize server message");
            (true, None)
        }
    }
}

fn log_writer_message(
    player_id: u32,
    message_kind: &'static str,
    snapshot_codec: &'static str,
    snapshot_codec_version: u16,
    frame_kind: &'static str,
    serialize: Duration,
    send: Duration,
    bytes: usize,
) {
    perf::log_writer_message(perf::WriterMessageTiming {
        player_id,
        message_kind,
        snapshot_codec,
        snapshot_codec_version,
        frame_kind,
        serialize,
        send,
        bytes,
    });
}

fn saturating_duration_ms_u32(duration: Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

enum ServerFrame {
    Text(String),
    Binary(Vec<u8>),
}

impl ServerFrame {
    fn from_snapshot_frame(frame: SnapshotFrame) -> Self {
        match frame {
            SnapshotFrame::Text(text) => ServerFrame::Text(text),
            SnapshotFrame::Binary(bytes) => ServerFrame::Binary(bytes),
        }
    }

    fn len(&self) -> usize {
        match self {
            ServerFrame::Text(text) => text.len(),
            ServerFrame::Binary(bytes) => bytes.len(),
        }
    }

    fn frame_kind(&self) -> &'static str {
        match self {
            ServerFrame::Text(_) => SNAPSHOT_FRAME_KIND_TEXT,
            ServerFrame::Binary(_) => rts_server::protocol::SNAPSHOT_FRAME_KIND_BINARY,
        }
    }

    fn into_message(self) -> Message {
        match self {
            ServerFrame::Text(text) => Message::Text(text.into()),
            ServerFrame::Binary(bytes) => Message::Binary(bytes.into()),
        }
    }
}
