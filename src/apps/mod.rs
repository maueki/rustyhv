mod fault;
mod hello;

pub(super) struct App {
    pub name: &'static str,
    pub entry: extern "C" fn() -> !,
}

pub(super) fn find(name: &str) -> Option<App> {
    match name {
        "hello" => Some(App {
            name: "hello",
            entry: hello_entry,
        }),
        "fault" => Some(App {
            name: "fault",
            entry: fault::run,
        }),
        _ => None,
    }
}

extern "C" fn hello_entry() -> ! {
    let status = match hello::run() {
        Ok(()) => 0,
        Err(error) => error.0,
    };
    crate::app_api::exit(status)
}
