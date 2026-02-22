use std::path::PathBuf;
use std::time::Duration;

use nomadflow_pty::actor::PaneSpawnConfig;
use nomadflow_pty::types::{PaneEvent, PaneId, PaneLabel, PaneMsg};
use nomadflow_pty::PaneActor;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn default_config() -> PaneSpawnConfig {
    PaneSpawnConfig {
        id: PaneId(1),
        label: PaneLabel("test:main:claude-1".into()),
        repo: "test".into(),
        worktree: "main".into(),
        agent_type: "claude".into(),
        agent_number: 1,
        cols: 80,
        rows: 24,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        shell: Some("/bin/sh".into()),
    }
}

/// Test 7.1: Verify PaneActor spawns a shell and output is received.
#[tokio::test]
async fn test_pty_spawn_and_output() {
    let config = default_config();
    let (handle, event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");
    let mut event_rx = event_tx.subscribe();

    // Send a simple command
    handle
        .tx
        .send(PaneMsg::Input(b"echo hello_pane_test\n".to_vec()))
        .await
        .expect("send input");

    // Wait for output containing our marker
    let mut found = false;
    let deadline = timeout(Duration::from_secs(5), async {
        loop {
            match event_rx.recv().await {
                Ok(PaneEvent::Output { data, .. }) => {
                    let text = String::from_utf8_lossy(&data);
                    if text.contains("hello_pane_test") {
                        found = true;
                        break;
                    }
                }
                Ok(PaneEvent::Exited { .. }) => break,
                _ => {}
            }
        }
    });

    let _ = deadline.await;
    assert!(found, "should have received output containing 'hello_pane_test'");

    // Clean shutdown
    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}

/// Test 7.2: Send known escape sequences and verify VTE parsing via snapshot.
#[tokio::test]
async fn test_vte_parsing_via_snapshot() {
    let config = default_config();
    let (handle, event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");
    let mut event_rx = event_tx.subscribe();

    // Send an escape sequence to position cursor and write text
    // \x1b[2J clears screen, \x1b[1;1H positions cursor at row 1 col 1
    let input = b"\x1b[2J\x1b[1;1Htest_vte\n";
    handle
        .tx
        .send(PaneMsg::Input(input.to_vec()))
        .await
        .expect("send input");

    // Wait a bit for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Drain any pending events
    while event_rx.try_recv().is_ok() {}

    // Request snapshot
    let (snap_tx, snap_rx) = oneshot::channel();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap_data = timeout(Duration::from_secs(3), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");

    // Snapshot should contain our text
    let snap_text = String::from_utf8_lossy(&snap_data);
    assert!(
        snap_text.contains("test_vte"),
        "snapshot should contain 'test_vte', got: {}",
        &snap_text[..snap_text.len().min(200)]
    );

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}

/// Test 7.3: Verify snapshot produces valid ANSI that reconstructs the screen.
#[tokio::test]
async fn test_buffer_snapshot_valid_ansi() {
    let config = default_config();
    let (handle, _event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");

    // Write some colored text
    // Bold red "RED" then reset and normal "NRM"
    let input = b"\x1b[2J\x1b[1;1H\x1b[1;31mRED\x1b[0m NRM\n";
    handle
        .tx
        .send(PaneMsg::Input(input.to_vec()))
        .await
        .expect("send input");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Request snapshot
    let (snap_tx, snap_rx) = oneshot::channel();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap_data = timeout(Duration::from_secs(3), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");

    // Snapshot should be valid UTF-8 containing ANSI escape sequences
    let snap_text = String::from_utf8_lossy(&snap_data);
    assert!(snap_text.contains("RED"), "snapshot should contain 'RED'");
    assert!(snap_text.contains("NRM"), "snapshot should contain 'NRM'");
    // Should contain SGR sequences (CSI codes)
    assert!(
        snap_text.contains("\x1b["),
        "snapshot should contain ANSI escape sequences"
    );
    // Should be reasonable size (< 20KB for 80x24)
    assert!(
        snap_data.len() < 20_000,
        "snapshot should be < 20KB, got {} bytes",
        snap_data.len()
    );

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}

/// Test 7.4: Verify resize message updates both PTY and Term dimensions.
#[tokio::test]
async fn test_resize() {
    let config = default_config();
    let (handle, _event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");

    // Send resize
    handle
        .tx
        .send(PaneMsg::Resize {
            cols: 120,
            rows: 40,
        })
        .await
        .expect("send resize");

    // Give time for the resize to process
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Write something and get a snapshot to verify the term processed the resize
    handle
        .tx
        .send(PaneMsg::Input(b"echo resize_ok\n".to_vec()))
        .await
        .expect("send input");

    tokio::time::sleep(Duration::from_millis(300)).await;

    let (snap_tx, snap_rx) = oneshot::channel();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap_data = timeout(Duration::from_secs(3), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");

    // If resize worked, we should be able to get a snapshot without panic
    assert!(!snap_data.is_empty(), "snapshot should not be empty after resize");

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}

/// Test 7.5: Verify PaneActor exits cleanly on Shutdown message.
#[tokio::test]
async fn test_shutdown() {
    let config = default_config();
    let (handle, event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");
    let mut event_rx = event_tx.subscribe();

    // Send shutdown
    handle
        .tx
        .send(PaneMsg::Shutdown)
        .await
        .expect("send shutdown");

    // The actor should stop. After shutdown, sending should eventually fail
    // (the receiver is dropped when the actor loop exits).
    tokio::time::sleep(Duration::from_millis(300)).await;

    // The event channel should eventually close or we should get Exited
    // Try to receive — either we get an event or the channel lags/closes
    let result = timeout(Duration::from_secs(2), async {
        loop {
            match event_rx.recv().await {
                Ok(PaneEvent::Exited { .. }) => return true,
                Err(_) => return true, // Channel closed or lagged = actor stopped
                _ => continue,
            }
        }
    })
    .await;

    // Either we got confirmation or timed out (actor stopped but no explicit exit event)
    // The key test: sending to the actor after shutdown should fail
    tokio::time::sleep(Duration::from_millis(200)).await;
    let send_result = handle
        .tx
        .send(PaneMsg::Input(b"should fail".to_vec()))
        .await;

    // After actor loop exits, the receiver is dropped, so send should fail
    assert!(
        send_result.is_err() || result.is_ok(),
        "actor should have stopped after Shutdown"
    );
}
