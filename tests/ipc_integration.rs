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
}

#[test]
fn ipc_json_rejects_unknown_command() {
    let result = serde_json::from_str::<IpcCommand>(r#"{"cmd":"explode"}"#);
    assert!(result.is_err());
}
