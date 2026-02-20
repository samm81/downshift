use breath_ball::IpcCommand;

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

    let move_window: IpcCommand = serde_json::from_str(r#"{"cmd":"move_window","x":12,"y":34}"#)
        .expect("move_window should parse");
    assert!(matches!(
        move_window,
        IpcCommand::MoveWindow { x: 12, y: 34 }
    ));
}

#[test]
fn ipc_json_rejects_unknown_command() {
    let result = serde_json::from_str::<IpcCommand>(r#"{"cmd":"explode"}"#);
    assert!(result.is_err());
}
