// Copyright 2022 - 2024 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2024/01/08 10:59:59

use tokio::{time::{Duration, Instant, sleep_until}, sync::mpsc::Receiver};
use webparse::ws::OwnedMessage;



// 存储由on_open返回的配置文件, 如定时器之类等
#[derive(Debug)]
pub struct WsOption {
    pub interval: Duration,
    pub receiver: Option<Receiver<OwnedMessage>>,
    next_interval: Instant,
}

impl WsOption {
    pub fn new(interval: Duration) -> Self {
        assert!(interval > Duration::from_micros(1));
        Self {
            interval,
            receiver: None,
            next_interval: Instant::now() + interval,
        }
    }

    pub fn set_receiver(&mut self, receiver: Receiver<OwnedMessage>) {
        self.receiver = Some(receiver);
    }

    async fn inner_interval_wait(&mut self) -> Option<()> {
        sleep_until(self.next_interval).await;
        self.next_interval = Instant::now() + self.interval;
        Some(())
    }

    pub async fn interval_wait(option: &mut Option<WsOption>) -> Option<()> {
        if option.is_some() {
            option.as_mut().unwrap().inner_interval_wait().await
        } else {
            let pend = std::future::pending();
            let () = pend.await;
            None
        }
    }
}
