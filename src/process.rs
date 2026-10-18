use crate::{apps::App, el0};

#[derive(Clone, Copy)]
pub(super) enum ExitReason {
    Code(u64),
    Fault { esr: u64 },
}

#[derive(Clone, Copy)]
pub(super) enum State {
    Ready,
    Running,
    Exited(ExitReason),
}

pub(super) struct Process {
    pub pid: u64,
    pub name: &'static str,
    pub state: State,
}

// 同期実行なのでスケジューラーは持たない。終了済みの記録は直近1件だけ保持する。
pub(super) struct Manager {
    next_pid: u64,
    last: Option<Process>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            next_pid: 1,
            last: None,
        }
    }

    pub fn last(&self) -> Option<&Process> {
        self.last.as_ref()
    }

    pub fn run(&mut self, app: App) -> Result<ExitReason, &'static str> {
        if self
            .last
            .as_ref()
            .is_some_and(|p| !matches!(p.state, State::Exited(_)))
        {
            return Err("A process is already active\r\n");
        }
        let pid = self.next_pid;
        self.next_pid = pid.checked_add(1).ok_or("PID exhausted\r\n")?;
        self.last = Some(Process {
            pid,
            name: app.name,
            state: State::Ready,
        });
        let process = self.last.as_mut().expect("created process");
        process.state = State::Running;
        // CPU コンテキストは el0.S が EL1 スタックに保存し、終了時に復元する。
        let result = el0::run(app.selector);
        let reason = if result.reason & el0::FAULT_BIT != 0 {
            ExitReason::Fault {
                esr: result.reason & !el0::FAULT_BIT,
            }
        } else {
            ExitReason::Code(result.status)
        };
        process.state = State::Exited(reason);
        Ok(reason)
    }
}
