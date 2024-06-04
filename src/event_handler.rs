use std::{
    fs::OpenOptions,
    os::{
        fd::{AsFd, OwnedFd},
        unix::prelude::{IntoRawFd, OpenOptionsExt},
    },
    path::Path,
    sync::{Arc, RwLock},
};

use chrono::Duration;
use input::{
    event::{
        gesture::{
            GestureEndEvent, GestureEventCoordinates, GestureEventTrait, GestureHoldEvent,
            GesturePinchEvent, GesturePinchEventTrait, GestureSwipeEvent,
        },
        Event, EventTrait, GestureEvent,
    },
    DeviceCapability, Libinput, LibinputInterface,
};
use miette::{miette, Result};
use nix::{
    fcntl::OFlag,
    poll::{poll, PollFd, PollFlags},
};
// use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::gestures::{hold::*, pinch::*, swipe::*, *};
use crate::utils::{exec_command, ExecArgs};
use crate::exec_handler::ExecHandler;

#[derive(Debug)]
pub struct EventHandler {
    config: Arc<RwLock<Config>>,
    event: Gesture,
    exec_handler: ExecHandler,
    swipe_gesture_delayed: bool,
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Self {
            config,
            event: Gesture::None,
            exec_handler: ExecHandler::new(),
            swipe_gesture_delayed: false,
        }
    }

    pub fn init(&mut self, input: &mut Libinput) -> Result<()> {
        log::debug!("{:?}  {:?}", &self, &input);
        self.init_ctx(input).expect("Could not initialize libinput");
        if self.has_gesture_device(input) {
            Ok(())
        } else {
            Err(miette!("Could not find gesture device"))
        }
    }

    fn init_ctx(&mut self, input: &mut Libinput) -> Result<(), ()> {
        input.udev_assign_seat("seat0")?;
        Ok(())
    }

    fn has_gesture_device(&mut self, input: &mut Libinput) -> bool {
        let mut found = false;
        log::debug!("Looking for gesture device");
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Device(e) = event {
                log::debug!("Device: {:?}", &e);
                found = e.device().has_capability(DeviceCapability::Gesture);
                log::debug!("Supports gestures: {:?}", found);
                if found {
                    return found;
                }
            } else {
                continue;
            }
            input.dispatch().unwrap();
        }
        found
    }

    pub fn main_loop(&mut self, input: &mut Libinput) {
        let mut cloned = input.clone();
        let fd = input.as_fd();
        let fds = PollFd::new(&fd, PollFlags::POLLIN);
        while poll(&mut [fds], -1).is_ok() {
            self.handle_event(&mut cloned)
                .expect("An Error occurred while handling an event");
        }
    }

    pub fn handle_event(&mut self, input: &mut Libinput) -> Result<()> {
        input.dispatch().unwrap();
        for event in input.clone() {
            if let Event::Gesture(e) = event {
                log::debug!("{:?}", &e);
                match e {
                    GestureEvent::Pinch(e) => self.handle_pinch_event(e)?,
                    GestureEvent::Swipe(e) => self.handle_swipe_event(e)?,
                    GestureEvent::Hold(e) => self.handle_hold_event(e)?,
                    _ => (),
                }
            } else if self.swipe_gesture_delayed {
                self.end_swipe_gesture();
            }
            input.dispatch().unwrap();
        }
        Ok(())
    }

    fn handle_hold_event(&mut self, event: GestureHoldEvent) -> Result<()> {
        match event {
            GestureHoldEvent::Begin(e) => {
                self.event = Gesture::Hold(Hold {
                    fingers: e.finger_count(),
                    action: None,
                })
            }
            GestureHoldEvent::End(_e) => {
                if let Gesture::Hold(s) = &self.event {
                    log::debug!("Hold: {:?}", &s.fingers);
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Hold(j) = i {
                            if j.fingers == s.fingers {
                                let exec_args = ExecArgs::new_args_only((&j.action).clone().unwrap_or_default());
                                exec_command(&exec_args)?;
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_pinch_event(&mut self, event: GesturePinchEvent) -> Result<()> {
        match event {
            GesturePinchEvent::Begin(e) => {
                self.event = Gesture::Pinch(Pinch {
                    fingers: e.finger_count(),
                    direction: PinchDir::Any,
                    update: None,
                    start: None,
                    end: None,
                });
                if let Gesture::Pinch(s) = &self.event {
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command(&ExecArgs::new_args_only(
                                    (&j.start).clone().unwrap_or_default()
                                ))?;
                            }
                        }
                    }
                }
            }
            GesturePinchEvent::Update(e) => {
                let scale = e.scale();
                let delta_angle = e.angle_delta();
                if let Gesture::Pinch(s) = &self.event {
                    let dir = PinchDir::dir(scale, delta_angle);
                    log::debug!(
                        "Pinch: scale={:?} angle={:?} direction={:?} fingers={:?}",
                        &scale,
                        &delta_angle,
                        &dir,
                        &s.fingers
                    );
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == dir || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command(&ExecArgs {
                                    args: (&j.update).clone().unwrap_or_default(),
                                    dx: 0.0,
                                    dy: 0.0,
                                    da: delta_angle,
                                    scale
                                })?;
                            }
                        }
                    }
                    self.event = Gesture::Pinch(Pinch {
                        fingers: s.fingers,
                        direction: dir,
                        update: None,
                        start: None,
                        end: None,
                    })
                }
            }
            GesturePinchEvent::End(_e) => {
                if let Gesture::Pinch(s) = &self.event {
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Pinch(j) = i {
                            if (j.direction == s.direction || j.direction == PinchDir::Any)
                                && j.fingers == s.fingers
                            {
                                exec_command(&ExecArgs::new_args_only(
                                    (&j.end).clone().unwrap_or_default(),
                                ))?;
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_swipe_event(&mut self, event: GestureSwipeEvent) -> Result<()> {
        match event {
            GestureSwipeEvent::Begin(e) => {
                self.event = Gesture::Swipe(Swipe {
                    direction: SwipeDir::Any,
                    fingers: e.finger_count(),
                    update: None,
                    start: None,
                    end: None,
                    allow_continue_delay: None,
                    include_cancelled: None,
                });
                if let Gesture::Swipe(s) = &self.event {
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Swipe(j) = i {
                            if j.fingers == s.fingers
                                && (j.direction == s.direction || j.direction == SwipeDir::Any)
                            {
                                if ! self.exec_handler.cancel_delay() {
                                    exec_command(&ExecArgs::new_args_only(
                                        (&j.start).clone().unwrap_or_default(),
                                    ))?;
                                }
                            }
                        }
                    }
                }
            }
            GestureSwipeEvent::Update(e) => {
                let (dx, dy) = (e.dx(), e.dy());
                let swipe_dir = SwipeDir::dir(dx, dy);

                if let Gesture::Swipe(s) = &self.event {
                    log::debug!("Swipe: direction={:?} fingers={:?}", &swipe_dir, &s.fingers);
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Swipe(j) = i {
                            if j.fingers == s.fingers
                                && (j.direction == swipe_dir || j.direction == SwipeDir::Any)
                            {
                                exec_command(&ExecArgs {
                                    args: (&j.update).clone().unwrap_or_default(),
                                    dx,
                                    dy,
                                    da: 0.0,
                                    scale: 0.0,
                                })?;
                            }
                        }
                    }
                    self.event = Gesture::Swipe(Swipe {
                        direction: swipe_dir,
                        fingers: s.fingers,
                        update: None,
                        start: None,
                        end: None,
                        allow_continue_delay: None,
                        include_cancelled: None,
                    })
                }
            }
            GestureSwipeEvent::End(e) => {
                if let Gesture::Swipe(s) = &self.event {
                    for i in &self.config.clone().read().unwrap().gestures {
                        if let Gesture::Swipe(j) = i {
                            if j.fingers == s.fingers
                                && (j.direction == s.direction || j.direction == SwipeDir::Any)
                            {
                                if !e.cancelled() || j.include_cancelled.unwrap_or_default() {
                                    let args = (&j.end).clone().unwrap_or_default();
                                    let delay = (&j.allow_continue_delay).clone().unwrap_or_default();
                                    self.exec_handler.schedule_command(
                                        Duration::milliseconds(delay),
                                        ExecArgs::new_args_only(args),
                                    );
                                    self.swipe_gesture_delayed = delay != 0;
                                }
                            }
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn end_swipe_gesture(&mut self) {
        self.exec_handler.exec_delayed_command();
        self.swipe_gesture_delayed = false;
    }
}

pub struct Interface;

impl LibinputInterface for Interface {
    fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
        OpenOptions::new()
            .custom_flags(flags)
            .read((false) | (flags & OFlag::O_RDWR.bits() != 0))
            .write((flags & OFlag::O_WRONLY.bits() != 0) | (flags & OFlag::O_RDWR.bits() != 0))
            .open(path)
            .map(|file| file.into())
            .map_err(|err| err.raw_os_error().unwrap())
    }
    fn close_restricted(&mut self, fd: OwnedFd) {
        nix::unistd::close(fd.into_raw_fd()).unwrap();
    }
}
