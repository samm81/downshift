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
        serde_json::from_str(r#"{"cmd":"start_drag"}"#).expect("start_drag should parse");
    assert!(matches!(start_drag, IpcCommand::StartDrag));
}

#[test]
fn ipc_json_rejects_unknown_command() {
    let result = serde_json::from_str::<IpcCommand>(r#"{"cmd":"explode"}"#);
    assert!(result.is_err());
}
