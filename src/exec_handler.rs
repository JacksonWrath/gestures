use timer::Timer;
use timer::Guard;
use chrono::Duration;
use std::{
    sync::mpsc,
    thread,
};
use crate::utils::{exec_command, ExecArgs};

pub enum ExecSignal {
    ScheduleDelay,
    ExecDelayed,
    ExecImmediate,
    CancelDelay,
    ClearDelayed,
}

struct ExecPayload {
    signal: ExecSignal,
    delay: Duration,
    exec_args: Option<ExecArgs>,
}

impl ExecPayload {
    fn new_signal_only(signal: ExecSignal) -> ExecPayload {
        Self {
            signal,
            delay: Duration::zero(),
            exec_args: None,
        }
    }
}

#[derive(Debug)]
pub struct ExecHandler {
    timer_tx: mpsc::Sender<ExecPayload>,
    timer_cancel_rx: mpsc::Receiver<bool>,
}

impl ExecHandler {
    pub fn new() -> Self {
        let (timer_tx, timer_rx) = mpsc::channel::<ExecPayload>();
        let (timer_cancel_tx, timer_cancel_rx) = mpsc::channel::<bool>();
        let tx_timer_thread = timer_tx.clone();
        thread::spawn(move || {
            let mut timer = TimerHandler::new();
            loop {
                let payload = timer_rx.recv().unwrap();
                match payload.signal {
                    ExecSignal::ScheduleDelay => {
                        let tx_schedule_clone = tx_timer_thread.clone();
                        timer.schedule_command(payload.delay, payload.exec_args.unwrap(), tx_schedule_clone);
                    }
                    ExecSignal::ExecDelayed => {
                        timer.exec_delayed_command();
                    }
                    ExecSignal::CancelDelay => {
                        let response = timer.cancel_delay();
                        timer_cancel_tx.send(response).unwrap();
                    }
                    ExecSignal::ClearDelayed => {
                        timer.clear_delayed_command();
                    }
                    ExecSignal::ExecImmediate => {
                        let _ = exec_command(&payload.exec_args.unwrap());
                    }
                }
            }
        });
        Self {
            timer_tx,
            timer_cancel_rx,
        }
    }

    pub fn schedule_command(&self, delay: Duration, exec_args: ExecArgs) {
        let payload = ExecPayload {
            signal: if delay.is_zero() { ExecSignal::ExecImmediate } else { ExecSignal::ScheduleDelay },
            delay,
            exec_args: Some(exec_args),
        };
        self.timer_tx.send(payload).unwrap();
    }

    pub fn exec_delayed_command(&self) {
        self.timer_tx.send(ExecPayload::new_signal_only(ExecSignal::ExecDelayed)).unwrap();
    }

    pub fn cancel_delay(&self) -> bool {
        self.timer_tx.send(ExecPayload::new_signal_only(ExecSignal::CancelDelay)).unwrap();
        self.timer_cancel_rx.recv().unwrap()
    }
}
struct TimerHandler {
    timer: Timer,
    guard: Option<Guard>,
    exec_args: Option<ExecArgs>,
}

impl TimerHandler {
    fn new() -> Self {
        TimerHandler {
            timer: Timer::new(),
            guard: None,
            exec_args: None,
        }
    }

    fn schedule_command(&mut self, delay: Duration, exec_args: ExecArgs, tx_clone: mpsc::Sender<ExecPayload>) {
        self.exec_args = Some(exec_args);
        let exec_args = (&self.exec_args).clone().unwrap();
        self.guard = Some(self.timer.schedule_with_delay(delay, move || {
            tx_clone.send(ExecPayload::new_signal_only(ExecSignal::ClearDelayed)).unwrap();
            let _ = exec_command(&exec_args);
        }));
    }

    fn exec_delayed_command(&mut self) {
        if self.exec_args.is_some() {
            let _ = exec_command(&self.exec_args.as_ref().unwrap());
        }
        self.cancel_delay();
    }

    fn cancel_delay(&mut self) -> bool {
        let something_was_cancelled = self.guard.is_some();
        self.guard = None;
        self.clear_delayed_command();
        something_was_cancelled
    }

    fn clear_delayed_command(&mut self) {
        self.exec_args = None;
    }
}