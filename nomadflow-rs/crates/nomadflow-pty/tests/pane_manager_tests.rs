use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use nomadflow_pty::types::{ClientId, CreatePaneRequest, PaneEvent, PaneId};
use nomadflow_pty::PaneManager;
use tokio::time::timeout;

fn test_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn make_request(repo: &str, worktree: &str, agent_type: &str) -> CreatePaneRequest {
    CreatePaneRequest {
        repo: repo.into(),
        worktree: worktree.into(),
        agent_type: agent_type.into(),
        cwd: test_cwd(),
        cols: None,
        rows: None,
        shell: Some("/bin/sh".into()),
    }
}

/// Test 11.1: create_pane spawns actor, assigns PaneId, returns correct PaneInfo with label.
#[tokio::test]
async fn test_create_pane() {
    let mut mgr = PaneManager::new();

    let info = mgr
        .create_pane(make_request("myapp", "main", "claude"))
        .expect("create_pane should succeed");

    assert_eq!(info.id, PaneId(1));
    assert_eq!(info.label.0, "myapp:main:claude-1");
    assert_eq!(info.cols, 80);
    assert_eq!(info.rows, 24);

    // Verify pane appears in list_panes
    let panes = mgr.list_panes();
    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0].id, PaneId(1));

    // Cleanup
    mgr.destroy_pane(PaneId(1)).unwrap();
}

/// Test 11.2: Agent numbering: same worktree increments, different worktree resets.
#[tokio::test]
async fn test_agent_numbering() {
    let mut mgr = PaneManager::new();

    // Two panes in same worktree
    let info1 = mgr
        .create_pane(make_request("myapp", "feat-auth", "claude"))
        .expect("create pane 1");
    let info2 = mgr
        .create_pane(make_request("myapp", "feat-auth", "opencode"))
        .expect("create pane 2");

    assert_eq!(info1.label.0, "myapp:feat-auth:claude-1");
    assert_eq!(info2.label.0, "myapp:feat-auth:opencode-2");

    // Pane in different worktree — agent number resets to 1
    let info3 = mgr
        .create_pane(make_request("myapp", "feat-pay", "claude"))
        .expect("create pane 3");

    assert_eq!(info3.label.0, "myapp:feat-pay:claude-1");

    // Cleanup
    for id in [PaneId(1), PaneId(2), PaneId(3)] {
        mgr.destroy_pane(id).unwrap();
    }
}

/// Test 11.3: list_panes returns all active panes.
#[tokio::test]
async fn test_list_panes() {
    let mut mgr = PaneManager::new();

    mgr.create_pane(make_request("a", "main", "claude")).unwrap();
    mgr.create_pane(make_request("b", "main", "claude")).unwrap();
    mgr.create_pane(make_request("c", "main", "claude")).unwrap();

    let panes = mgr.list_panes();
    assert_eq!(panes.len(), 3);

    let ids: HashSet<PaneId> = panes.iter().map(|p| p.id).collect();
    assert!(ids.contains(&PaneId(1)));
    assert!(ids.contains(&PaneId(2)));
    assert!(ids.contains(&PaneId(3)));

    // Cleanup
    for id in [PaneId(1), PaneId(2), PaneId(3)] {
        mgr.destroy_pane(id).unwrap();
    }
}

/// Test 11.4: destroy_pane removes from list, second destroy returns PaneNotFound.
#[tokio::test]
async fn test_destroy_pane() {
    let mut mgr = PaneManager::new();

    mgr.create_pane(make_request("myapp", "main", "claude")).unwrap();
    assert_eq!(mgr.list_panes().len(), 1);

    mgr.destroy_pane(PaneId(1)).unwrap();
    assert_eq!(mgr.list_panes().len(), 0);

    // Second destroy should return PaneNotFound
    let result = mgr.destroy_pane(PaneId(1));
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found"), "error should be PaneNotFound, got: {}", err_msg);
}

/// Test 11.5: route_input sends data to the pane's PTY.
#[tokio::test]
async fn test_route_input() {
    let mut mgr = PaneManager::new();

    let info = mgr
        .create_pane(make_request("myapp", "main", "claude"))
        .expect("create pane");

    // Subscribe to get output
    let mut rx = mgr
        .subscribe_client(ClientId(1), info.id)
        .expect("subscribe");

    // Route input: echo command
    mgr.route_input(info.id, b"echo route_test_marker\n".to_vec())
        .await
        .expect("route_input");

    // Wait for output containing our marker
    let mut found = false;
    let _ = timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Ok(PaneEvent::Output { data, .. }) => {
                    if String::from_utf8_lossy(&data).contains("route_test_marker") {
                        found = true;
                        break;
                    }
                }
                Ok(PaneEvent::Exited { .. }) => break,
                _ => {}
            }
        }
    })
    .await;

    assert!(found, "should have received output containing 'route_test_marker'");

    mgr.destroy_pane(info.id).unwrap();
}

/// Test 11.6: get_buffer_snapshot returns non-empty bytes.
#[tokio::test]
async fn test_get_buffer_snapshot() {
    let mut mgr = PaneManager::new();

    let info = mgr
        .create_pane(make_request("myapp", "main", "claude"))
        .expect("create pane");

    // Poll for non-empty snapshot with timeout (avoids flaky sleep)
    let mut snapshot = Vec::new();
    let _ = timeout(Duration::from_secs(5), async {
        loop {
            snapshot = mgr
                .get_buffer_snapshot(info.id)
                .await
                .expect("get_buffer_snapshot");
            if !snapshot.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;

    assert!(!snapshot.is_empty(), "snapshot should be non-empty");

    mgr.destroy_pane(info.id).unwrap();
}

/// Test 11.7: subscribe/unsubscribe client tracking and broadcast receiver.
#[tokio::test]
async fn test_subscribe_unsubscribe_client() {
    let mut mgr = PaneManager::new();

    let info1 = mgr.create_pane(make_request("a", "main", "claude")).unwrap();
    let info2 = mgr.create_pane(make_request("b", "main", "claude")).unwrap();

    let client = ClientId(42);

    // Subscribe to both panes
    let _rx1 = mgr.subscribe_client(client, info1.id).expect("subscribe pane 1");
    let _rx2 = mgr.subscribe_client(client, info2.id).expect("subscribe pane 2");

    let subs = mgr.get_client_subscriptions(client);
    assert_eq!(subs.len(), 2);
    assert!(subs.contains(&info1.id));
    assert!(subs.contains(&info2.id));

    // Unsubscribe from one
    mgr.unsubscribe_client(client, info1.id);
    let subs = mgr.get_client_subscriptions(client);
    assert_eq!(subs.len(), 1);
    assert!(subs.contains(&info2.id));

    // Non-existent client returns empty set
    let empty = mgr.get_client_subscriptions(ClientId(999));
    assert!(empty.is_empty());

    // Cleanup
    mgr.destroy_pane(info1.id).unwrap();
    mgr.destroy_pane(info2.id).unwrap();
}

/// Test 11.8: remove_client clears all subscriptions for that client.
#[tokio::test]
async fn test_remove_client() {
    let mut mgr = PaneManager::new();

    let info1 = mgr.create_pane(make_request("a", "main", "claude")).unwrap();
    let info2 = mgr.create_pane(make_request("b", "main", "claude")).unwrap();
    let info3 = mgr.create_pane(make_request("c", "main", "claude")).unwrap();

    let client = ClientId(7);

    // Subscribe to all three panes
    let _rx1 = mgr.subscribe_client(client, info1.id).unwrap();
    let _rx2 = mgr.subscribe_client(client, info2.id).unwrap();
    let _rx3 = mgr.subscribe_client(client, info3.id).unwrap();

    assert_eq!(mgr.get_client_subscriptions(client).len(), 3);

    // Remove client entirely
    mgr.remove_client(client);

    assert!(mgr.get_client_subscriptions(client).is_empty());

    // Cleanup
    mgr.destroy_pane(info1.id).unwrap();
    mgr.destroy_pane(info2.id).unwrap();
    mgr.destroy_pane(info3.id).unwrap();
}

/// Test: route_input to non-existent pane returns PaneNotFound.
#[tokio::test]
async fn test_route_input_pane_not_found() {
    let mgr = PaneManager::new();

    let result = mgr.route_input(PaneId(999), b"hello".to_vec()).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found"));
}

/// Test: get_buffer_snapshot for non-existent pane returns PaneNotFound.
#[tokio::test]
async fn test_snapshot_pane_not_found() {
    let mgr = PaneManager::new();

    let result = mgr.get_buffer_snapshot(PaneId(999)).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found"));
}
