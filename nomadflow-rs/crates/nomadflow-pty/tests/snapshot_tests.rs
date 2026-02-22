use std::path::PathBuf;
use std::time::{Duration, Instant};

use nomadflow_pty::actor::PaneSpawnConfig;
use nomadflow_pty::snapshot::{POSTAMBLE_SHOW_CURSOR, PREAMBLE};
use nomadflow_pty::types::{PaneId, PaneLabel, PaneMsg};
use nomadflow_pty::PaneActor;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn config_80x24(id: u16) -> PaneSpawnConfig {
    PaneSpawnConfig {
        id: PaneId(id),
        label: PaneLabel(format!("test:main:snap-{id}")),
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

fn config_120x40(id: u16) -> PaneSpawnConfig {
    PaneSpawnConfig {
        id: PaneId(id),
        label: PaneLabel(format!("test:main:snap-{id}")),
        repo: "test".into(),
        worktree: "main".into(),
        agent_type: "claude".into(),
        agent_number: 1,
        cols: 120,
        rows: 40,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        shell: Some("/bin/sh".into()),
    }
}

/// Helper: spawn a PaneActor, send printf command to produce ANSI output, take snapshot.
/// Uses `printf` to generate real escape sequences via stdout (VTE-processed).
async fn snapshot_with_printf(config: PaneSpawnConfig, printf_arg: &str) -> Vec<u8> {
    let (handle, _event_tx, _join) = PaneActor::spawn(config).expect("spawn should succeed");

    // Wait for shell to start and show prompt
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Use printf to output escape sequences properly (processed by VTE)
    let cmd = format!("printf '{printf_arg}'\n");
    handle
        .tx
        .send(PaneMsg::Input(cmd.into_bytes()))
        .await
        .expect("send input");

    // Wait for printf execution + VTE processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    let (snap_tx, snap_rx) = oneshot::channel();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap = timeout(Duration::from_secs(5), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
    snap
}

// ============================================================================
// Task 7: ANSI Fidelity Integration Tests (AC: #2)
// ============================================================================

/// Test 7.1: Bold + color fidelity
#[tokio::test]
async fn test_fidelity_bold_color() {
    let snap = snapshot_with_printf(
        config_80x24(200),
        "\\033[2J\\033[1;1H\\033[1;31mBOLD RED\\033[0m",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("BOLD RED"), "should contain text 'BOLD RED'");
    assert!(text.contains("\x1b[1m"), "should contain bold SGR");
    assert!(
        text.contains("\x1b[31m") || text.contains("\x1b[91m"),
        "should contain red or bright-red fg SGR"
    );
}

/// Test 7.2: 256-color fidelity
#[tokio::test]
async fn test_fidelity_256_color() {
    let snap = snapshot_with_printf(
        config_80x24(201),
        "\\033[2J\\033[1;1H\\033[38;5;196mIDX196\\033[0m",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("IDX196"), "should contain text 'IDX196'");
    assert!(
        text.contains("\x1b[38;5;196m"),
        "should contain 256-color escape"
    );
}

/// Test 7.3: Truecolor fidelity
#[tokio::test]
async fn test_fidelity_truecolor() {
    let snap = snapshot_with_printf(
        config_80x24(202),
        "\\033[2J\\033[1;1H\\033[38;2;255;128;0mTRUE\\033[0m",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("TRUE"), "should contain text 'TRUE'");
    assert!(
        text.contains("\x1b[38;2;255;128;0m"),
        "should contain truecolor escape"
    );
}

/// Test 7.4: Inverse video fidelity
#[tokio::test]
async fn test_fidelity_inverse() {
    let snap = snapshot_with_printf(
        config_80x24(203),
        "\\033[2J\\033[1;1H\\033[7mINVERSE\\033[0m",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("INVERSE"), "should contain text 'INVERSE'");
    assert!(text.contains("\x1b[7m"), "should contain inverse SGR");
}

/// Test 7.5: Combined styles fidelity (bold + italic + underline + red)
#[tokio::test]
async fn test_fidelity_combined_styles() {
    let snap = snapshot_with_printf(
        config_80x24(204),
        "\\033[2J\\033[1;1H\\033[1;3;4;31mALL\\033[0m",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("ALL"), "should contain text 'ALL'");
    assert!(text.contains("\x1b[1m"), "should contain bold SGR");
    assert!(text.contains("\x1b[3m"), "should contain italic SGR");
    assert!(text.contains("\x1b[4m"), "should contain underline SGR");
    assert!(
        text.contains("\x1b[31m") || text.contains("\x1b[91m"),
        "should contain red or bright-red fg SGR"
    );
}

/// Test 7.6: Preamble present at start of every snapshot
#[tokio::test]
async fn test_preamble_present() {
    let snap = snapshot_with_printf(
        config_80x24(205),
        "\\033[2J\\033[1;1Hpreamble_test",
    )
    .await;

    assert!(
        snap.starts_with(PREAMBLE),
        "snapshot should start with preamble (hide cursor + clear + home + SGR reset)"
    );
}

/// Test 7.7: Postamble — snapshot ends with show cursor
#[tokio::test]
async fn test_postamble_show_cursor() {
    let snap = snapshot_with_printf(
        config_80x24(206),
        "\\033[2J\\033[1;1Hpostamble_test",
    )
    .await;

    assert!(
        snap.ends_with(POSTAMBLE_SHOW_CURSOR),
        "snapshot should end with show cursor sequence"
    );
}

// ============================================================================
// Task 2.4: Default foreground color explicit reset (AC: #1, #2)
// ============================================================================

/// Test 2.4: Cell with default foreground color emits \x1b[39m
#[tokio::test]
async fn test_default_fg_color_reset() {
    // Red text followed by SGR-reset then default-colored text.
    // The serializer must emit \x1b[39m for the default fg cells.
    let snap = snapshot_with_printf(
        config_80x24(209),
        "\\033[2J\\033[1;1H\\033[31mRED\\033[0mDEFAULT",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    assert!(text.contains("DEFAULT"), "should contain text 'DEFAULT'");
    assert!(
        text.contains("\x1b[39m"),
        "should contain default foreground SGR reset (\\x1b[39m)"
    );
}

// ============================================================================
// Task 4.3: Cursor shape DECSCUSR verification (AC: #2)
// ============================================================================

/// Test 4.3: Snapshot contains a DECSCUSR cursor shape sequence
#[tokio::test]
async fn test_cursor_shape_decscusr() {
    let snap = snapshot_with_printf(
        config_80x24(210),
        "\\033[2J\\033[1;1Hcursor_shape_test",
    )
    .await;
    let text = String::from_utf8_lossy(&snap);

    // Shell typically uses Block cursor → DECSCUSR \x1b[2 q
    // Accept any valid DECSCUSR: Block (2), Underline (4), or Beam (6)
    assert!(
        text.contains("\x1b[2 q") || text.contains("\x1b[4 q") || text.contains("\x1b[6 q"),
        "snapshot should contain a DECSCUSR cursor shape sequence"
    );
}

// ============================================================================
// Task 6: Performance Benchmarks (AC: #3)
// ============================================================================

/// Test 6.1-6.3: Snapshot size and latency for 80x24 with realistic content
#[tokio::test]
async fn test_performance_80x24() {
    let (handle, _event_tx, _join) =
        PaneActor::spawn(config_80x24(207)).expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Build a printf command that produces realistic colored terminal content
    let mut printf_arg = String::new();
    printf_arg.push_str("\\033[2J\\033[1;1H"); // Clear + home

    // Colored shell prompt
    printf_arg.push_str("\\033[1;32muser@host\\033[0m:\\033[1;34m~/project\\033[0m$ ");

    // Simulated ls --color output
    printf_arg.push_str("\\033[0;34msrc/\\033[0m  ");
    printf_arg.push_str("\\033[0;32mREADME.md\\033[0m  ");
    printf_arg.push_str("\\033[0mCargo.toml  ");
    printf_arg.push_str("\\033[1;31mtarget.tar.gz\\033[0m\\n");

    // Add lines of mixed content
    for line in 1..10 {
        printf_arg.push_str(&format!(
            "\\033[33mline {line}\\033[0m: some text with \\033[1mbold\\033[0m\\n"
        ));
    }

    let cmd = format!("printf '{printf_arg}'\n");
    handle
        .tx
        .send(PaneMsg::Input(cmd.into_bytes()))
        .await
        .expect("send input");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Measure snapshot performance
    let (snap_tx, snap_rx) = oneshot::channel();
    let start = Instant::now();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap = timeout(Duration::from_secs(5), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");
    let elapsed = start.elapsed();

    // AC #3: size < 20KB for 80x24
    assert!(
        snap.len() < 20_000,
        "snapshot should be < 20KB for 80x24, got {} bytes",
        snap.len()
    );

    // Round-trip via PaneMsg::Snapshot < 50ms (generous for CI)
    assert!(
        elapsed < Duration::from_millis(50),
        "snapshot round-trip should be < 50ms, took {:?}",
        elapsed
    );

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}

/// Test 6.4: Verify reasonable scaling for 120x40 screen
#[tokio::test]
async fn test_performance_120x40() {
    let (handle, _event_tx, _join) =
        PaneActor::spawn(config_120x40(208)).expect("spawn should succeed");

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Clear screen first
    handle
        .tx
        .send(PaneMsg::Input(b"printf '\\033[2J\\033[1;1H'\n".to_vec()))
        .await
        .expect("send clear");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send content in batches (avoid exceeding terminal line buffer)
    for batch_start in (1u32..35).step_by(5) {
        let batch_end = (batch_start + 5).min(35);
        let mut printf_arg = String::new();
        for line in batch_start..batch_end {
            printf_arg.push_str(&format!(
                "\\033[36m{:>4}\\033[0m | \\033[32mfn ex_{line}\\033[0m() {{ return {}; }}\\n",
                line,
                line * 42
            ));
        }
        let cmd = format!("printf '{printf_arg}'\n");
        handle
            .tx
            .send(PaneMsg::Input(cmd.into_bytes()))
            .await
            .expect("send batch");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    let (snap_tx, snap_rx) = oneshot::channel();
    let start = Instant::now();
    handle
        .tx
        .send(PaneMsg::Snapshot(snap_tx))
        .await
        .expect("send snapshot request");

    let snap = timeout(Duration::from_secs(5), snap_rx)
        .await
        .expect("snapshot timeout")
        .expect("snapshot channel");
    let elapsed = start.elapsed();

    // For 120x40, allow ~3x the 80x24 limit
    assert!(
        snap.len() < 60_000,
        "snapshot should be < 60KB for 120x40, got {} bytes",
        snap.len()
    );

    assert!(
        elapsed < Duration::from_millis(100),
        "snapshot round-trip should be < 100ms for 120x40, took {:?}",
        elapsed
    );

    let _ = handle.tx.send(PaneMsg::Shutdown).await;
}
