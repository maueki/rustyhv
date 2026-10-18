pub(super) struct App {
    pub name: &'static str,
    pub selector: u64,
}

pub(super) fn find(name: &str) -> Option<App> {
    match name {
        "hello" => Some(App {
            name: "hello",
            selector: 0,
        }),
        "fault" => Some(App {
            name: "fault",
            selector: 1,
        }),
        "read-kernel" => Some(App {
            name: "read-kernel",
            selector: 2,
        }),
        "write-code" => Some(App {
            name: "write-code",
            selector: 3,
        }),
        "exec-stack" => Some(App {
            name: "exec-stack",
            selector: 4,
        }),
        "write-kernel" => Some(App {
            name: "write-kernel",
            selector: 5,
        }),
        "read-device" => Some(App {
            name: "read-device",
            selector: 6,
        }),
        "stack-guard" => Some(App {
            name: "stack-guard",
            selector: 7,
        }),
        _ => None,
    }
}
