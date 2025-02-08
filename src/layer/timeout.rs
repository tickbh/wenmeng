// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/11/10 02:23:05

use std::{
    pin::Pin,
    task::Context,
    time::{Duration, Instant},
};

use std::future::Future;
use tokio::time::Sleep;

use crate::ProtResult;

pub struct TimeoutLayer {
    pub connect_timeout: Option<Duration>,
    pub read_timeout: Option<Duration>,
    pub write_timeout: Option<Duration>,
    pub timeout: Option<Duration>,
    /// keep alive 超时时长
    pub ka_timeout: Option<Duration>,

    read_timeout_sleep: Option<Pin<Box<Sleep>>>,
    write_timeout_sleep: Option<Pin<Box<Sleep>>>,
    timeout_sleep: Option<Pin<Box<Sleep>>>,
    ka_timeout_sleep: Option<Pin<Box<Sleep>>>,
}

impl Clone for TimeoutLayer {
    fn clone(&self) -> Self {
        Self {
            connect_timeout: self.connect_timeout.clone(),
            read_timeout: self.read_timeout.clone(),
            write_timeout: self.write_timeout.clone(),
            timeout: self.timeout.clone(),
            ka_timeout: self.ka_timeout.clone(),
            read_timeout_sleep: None,
            write_timeout_sleep: None,
            timeout_sleep: None,
            ka_timeout_sleep: None,

        }
    }
}

impl TimeoutLayer {
    pub fn new() -> Self {
        Self {
            connect_timeout: None,
            read_timeout: None,
            write_timeout: None,
            timeout: None,
            ka_timeout: None,

            read_timeout_sleep: None,
            write_timeout_sleep: None,
            timeout_sleep: None,
            ka_timeout_sleep: None,
        }
    }
    
    pub fn set_connect_timeout(&mut self, connect_timeout: Option<Duration>) {
        self.connect_timeout = connect_timeout;
    }

    pub fn set_read_timeout(&mut self, read_timeout: Option<Duration>) {
        self.read_timeout = read_timeout;
    }

    pub fn set_write_timeout(&mut self, write_timeout: Option<Duration>) {
        self.write_timeout = write_timeout;
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub fn set_ka_timeout(&mut self, ka_timeout: Option<Duration>) {
        self.ka_timeout = ka_timeout;
    }

    pub fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
        info: &'static str,
        ready_time: Instant,
        is_read_end: bool,
        is_write_end: bool,
        is_idle: bool,
    ) -> ProtResult<()> {
        let now = Instant::now();
        if !is_read_end {
            if let Some(read) = &self.read_timeout {
                let next = ready_time + *read;
                if now >= next {
                    return Err(crate::ProtError::read_timeout(info));
                }
                if self.read_timeout_sleep.is_some() {
                    self.read_timeout_sleep.as_mut().unwrap().as_mut().set(tokio::time::sleep_until(next.into()));
                } else {
                    self.read_timeout_sleep = Some(Box::pin(tokio::time::sleep_until(next.into())));
                }
                let _ = Pin::new(self.read_timeout_sleep.as_mut().unwrap()).poll(cx);
            }
        }
        if !is_write_end {
            if let Some(write) = &self.write_timeout {
                let next = ready_time + *write;
                if now >= next {
                    return Err(crate::ProtError::write_timeout(info));
                }
                if self.write_timeout_sleep.is_some() {
                    self.write_timeout_sleep.as_mut().unwrap().as_mut().set(tokio::time::sleep_until(next.into()));
                } else {
                    self.write_timeout_sleep = Some(Box::pin(tokio::time::sleep_until(next.into())));
                }
                let _ = Pin::new(self.write_timeout_sleep.as_mut().unwrap()).poll(cx);
            }
        }

        if !is_read_end && !is_write_end {
            if let Some(time) = &self.timeout {
                let next = ready_time + *time;
                if now >= next {
                    return Err(crate::ProtError::time_timeout(info));
                }
                if self.timeout_sleep.is_some() {
                    self.timeout_sleep.as_mut().unwrap().as_mut().set(tokio::time::sleep_until(next.into()));
                } else {
                    self.timeout_sleep = Some(Box::pin(tokio::time::sleep_until(next.into())));
                }
                let _ = Pin::new(self.timeout_sleep.as_mut().unwrap()).poll(cx);
            }
        }

        if is_idle {
            if let Some(time) = &self.ka_timeout {
                let next = ready_time + *time;
                if now >= next {
                    return Err(crate::ProtError::ka_timeout(info));
                }
                if self.ka_timeout_sleep.is_some() {
                    self.ka_timeout_sleep.as_mut().unwrap().as_mut().set(tokio::time::sleep_until(next.into()));
                } else {
                    self.ka_timeout_sleep = Some(Box::pin(tokio::time::sleep_until(next.into())));
                }
                let _ = Pin::new(self.ka_timeout_sleep.as_mut().unwrap()).poll(cx);
            }
        }

        Ok(())
    }
}
