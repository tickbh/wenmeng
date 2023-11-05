use std::{
    pin::Pin,
    task::Context,
    time::{Duration, Instant},
};

use futures_core::Future;

use crate::ProtResult;

#[derive(Debug)]
pub struct TimeoutLayer {
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    timeout: Option<Duration>,
}

impl TimeoutLayer {
    pub fn new() -> Self {
        Self {
            read_timeout: None,
            write_timeout: None,
            timeout: None,
        }
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

    pub fn poll_ready(
        &mut self,
        cx: &mut Context<'_>,
        ready_time: Instant,
        is_read_end: bool,
        is_write_end: bool,
    ) -> ProtResult<()> {
        let now = Instant::now();
        println!("aaaaaaaaa");
        if !is_read_end {
            println!("aaaaaaaaa read_timeout = {:?}", self.read_timeout);
            if let Some(read) = &self.read_timeout {
                let next = ready_time + *read;
                if now > next {
                    return Err(crate::ProtError::Extension("read timeout"));
                }

                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);
            }
        }
        if !is_write_end {
            if let Some(write) = &self.write_timeout {
                let next = ready_time + *write;
                if now > next {
                    return Err(crate::ProtError::Extension("write timeout"));
                }
                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);
            }
        }

        if !is_read_end && !is_write_end {
            if let Some(time) = &self.timeout {
                let next = ready_time + *time;
                if now > next {
                    return Err(crate::ProtError::Extension("timeout"));
                }
                let _ = Pin::new(&mut Box::pin(tokio::time::sleep_until(next.into()))).poll(cx);
            }
        }

        Ok(())
    }
}
