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
        _ => None,
    }
}
