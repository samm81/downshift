use downshift::IpcCommand;

#[test]
fn ipc_json_deserializes_supported_commands() {
    let quit: IpcCommand = serde_json::from_str(r#"{"cmd":"quit"}"#).expect("quit should parse");
    assert!(matches!(quit, IpcCommand::Quit));

    let resize: IpcCommand = serde_json::from_str(r#"{"cmd":"resize","delta":-1,"fine":true}"#)
        .expect("resize should parse");
    assert!(matches!(
        resize,
        IpcCommand::Resize {
            delta: -1,
            fine: true
        }
    ));

    let context_menu: IpcCommand =
        serde_json::from_str(r#"{"cmd":"show_context_menu","x":24,"y":36}"#)
            .expect("show_context_menu should parse");
    assert!(matches!(
        context_menu,
        IpcCommand::ShowContextMenu { x: 24, y: 36 }
    ));

    let start_drag: IpcCommand =
        serde_json::from_str(r#"{"cmd":"start_drag","screen_x":80,"screen_y":160}"#)
            .expect("start_drag should parse");
    assert!(matches!(
        start_drag,
        IpcCommand::StartDrag {
            screen_x: 80,
            screen_y: 160
        }
    ));

    let drag_to: IpcCommand =
        serde_json::from_str(r#"{"cmd":"drag_to","screen_x":96,"screen_y":192}"#)
            .expect("drag_to should parse");
    assert!(matches!(
        drag_to,
        IpcCommand::DragTo {
            screen_x: 96,
            screen_y: 192
        }
    ));

    let end_drag: IpcCommand =
        serde_json::from_str(r#"{"cmd":"end_drag"}"#).expect("end_drag should parse");
    assert!(matches!(end_drag, IpcCommand::EndDrag));

    let set_usage: IpcCommand =
        serde_json::from_str(r#"{"cmd":"set_usage_data_sharing","enabled":true}"#)
            .expect("set_usage_data_sharing should parse");
    assert!(matches!(
        set_usage,
        IpcCommand::SetUsageDataSharing { enabled: true }
    ));

    let set_crash: IpcCommand =
        serde_json::from_str(r#"{"cmd":"set_crash_reports_sharing","enabled":false}"#)
            .expect("set_crash_reports_sharing should parse");
    assert!(matches!(
        set_crash,
        IpcCommand::SetCrashReportsSharing { enabled: false }
    ));

    let analytics_opened: IpcCommand = serde_json::from_str(r#"{"cmd":"analytics_menu_opened"}"#)
        .expect("analytics_menu_opened should parse");
    assert!(matches!(analytics_opened, IpcCommand::AnalyticsMenuOpened));

    let show_telemetry_info: IpcCommand = serde_json::from_str(r#"{"cmd":"show_telemetry_info"}"#)
        .expect("show_telemetry_info should parse");
    assert!(matches!(show_telemetry_info, IpcCommand::ShowTelemetryInfo));

    let close_telemetry_info: IpcCommand =
        serde_json::from_str(r#"{"cmd":"close_telemetry_info"}"#)
            .expect("close_telemetry_info should parse");
    assert!(matches!(
        close_telemetry_info,
        IpcCommand::CloseTelemetryInfo
    ));

    let set_snooze: IpcCommand = serde_json::from_str(r#"{"cmd":"set_snooze","minutes":15}"#)
        .expect("set_snooze should parse");
    assert!(matches!(set_snooze, IpcCommand::SetSnooze { minutes: 15 }));

    let show_breathing_pattern: IpcCommand =
        serde_json::from_str(r#"{"cmd":"show_breathing_pattern"}"#)
            .expect("show_breathing_pattern should parse");
    assert!(matches!(
        show_breathing_pattern,
        IpcCommand::ShowBreathingPattern
    ));

    let close_breathing_pattern: IpcCommand =
        serde_json::from_str(r#"{"cmd":"close_breathing_pattern"}"#)
            .expect("close_breathing_pattern should parse");
    assert!(matches!(
        close_breathing_pattern,
        IpcCommand::CloseBreathingPattern
    ));

    let apply_breathing_pattern: IpcCommand = serde_json::from_str(
        r#"{"cmd":"apply_breathing_pattern","preset_id":"custom","pattern":{"expanding_seconds":4.0,"expanded_hold_seconds":7.0,"compressing_seconds":9.0,"compressed_hold_seconds":0.0}}"#,
    )
    .expect("apply_breathing_pattern should parse");
    assert!(matches!(
        apply_breathing_pattern,
        IpcCommand::ApplyBreathingPattern { .. }
    ));

    let save_breathing_preset: IpcCommand = serde_json::from_str(
        r#"{"cmd":"save_breathing_preset","name":"focus","pattern":{"expanding_seconds":4.0,"expanded_hold_seconds":2.0,"compressing_seconds":6.0,"compressed_hold_seconds":2.0}}"#,
    )
    .expect("save_breathing_preset should parse");
    assert!(matches!(
        save_breathing_preset,
        IpcCommand::SaveBreathingPreset { .. }
    ));

    let show_custom_snooze: IpcCommand = serde_json::from_str(r#"{"cmd":"show_custom_snooze"}"#)
        .expect("show_custom_snooze should parse");
    assert!(matches!(show_custom_snooze, IpcCommand::ShowCustomSnooze));

    let close_custom_snooze: IpcCommand = serde_json::from_str(r#"{"cmd":"close_custom_snooze"}"#)
        .expect("close_custom_snooze should parse");
    assert!(matches!(close_custom_snooze, IpcCommand::CloseCustomSnooze));

    let update_primary_action: IpcCommand =
        serde_json::from_str(r#"{"cmd":"update_primary_action"}"#)
            .expect("update_primary_action should parse");
    assert!(matches!(
        update_primary_action,
        IpcCommand::UpdatePrimaryAction
    ));

    let dismiss_update_badge: IpcCommand =
        serde_json::from_str(r#"{"cmd":"dismiss_update_badge"}"#)
            .expect("dismiss_update_badge should parse");
    assert!(matches!(
        dismiss_update_badge,
        IpcCommand::DismissUpdateBadge
    ));

    let close_update_dialog: IpcCommand = serde_json::from_str(r#"{"cmd":"close_update_dialog"}"#)
        .expect("close_update_dialog should parse");
    assert!(matches!(close_update_dialog, IpcCommand::CloseUpdateDialog));

    let download_update: IpcCommand =
        serde_json::from_str(r#"{"cmd":"download_update"}"#).expect("download_update should parse");
    assert!(matches!(download_update, IpcCommand::DownloadUpdate));
}

#[test]
fn ipc_json_rejects_unknown_command() {
    let result = serde_json::from_str::<IpcCommand>(r#"{"cmd":"explode"}"#);
    assert!(result.is_err());
}
