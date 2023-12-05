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

use std::fmt::Display;
use std::future::Future;
use std::str::FromStr;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::{Duration, Instant, Sleep};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// 周期内可以通行的数据
    pub nums: u64,
    /// 每个周期的时间
    pub per: Duration,
}

impl Rate {
    pub fn new(nums: u64, per: Duration) -> Self {
        Self { nums, per }
    }
}

impl Default for Rate {
    fn default() -> Self {
        Self {
            nums: Default::default(),
            per: Default::default(),
        }
    }
}

impl Display for Rate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ms = self.per.subsec_millis();
        let s = self.per.as_secs();
        if ms > 0 {
            f.write_str(&format!("{}ms", ms as u64 + s * 1000))
        } else {
            if s >= 3600 && s % 3600 == 0 {
                f.write_str(&format!("{}h", s / 3600))
            } else if s >= 60 && s % 60 == 0 {
                f.write_str(&format!("{}min", s / 60))
            } else {
                f.write_str(&format!("{}s", s))
            }
        }
    }
}

#[derive(Debug)]
pub struct RateLimitLayer {
    /// 限速频率
    rate: Rate,
    /// 当前周期下，还剩下可通行的数据
    left_nums: u64,
    /// 下一个时间重新计算的日期
    util: Instant,
    sleep: Pin<Box<Sleep>>,
}

impl RateLimitLayer {
    pub fn new(rate: Rate) -> Self {
        let util = Instant::now();
        Self {
            left_nums: rate.nums,
            rate,
            util,
            sleep: Box::pin(tokio::time::sleep_until(util)),
        }
    }

    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.left_nums > 0 {
            return Poll::Ready(Ok(()));
        }

        if Pin::new(&mut self.sleep).poll(cx).is_pending() {
            tracing::trace!("rate limit exceeded; sleeping.");
            return Poll::Pending;
        }

        self.left_nums = self.rate.nums;
        self.util = Instant::now() + self.rate.per;
        self.sleep
            .as_mut()
            .set(tokio::time::sleep_until(Instant::now() + self.rate.per));
        return Poll::Ready(Ok(()));
    }

    pub fn poll_call(&mut self, mut count: u64) -> io::Result<()> {
        if self.left_nums == 0 {
            return Ok(());
        }

        let now = Instant::now();
        if now > self.util {
            self.rate.nums = self.left_nums;
            self.util = now + self.rate.per;
            // self.sleep.as_mut().set(tokio::time::sleep_until(self.util));
        }

        if self.left_nums > count {
            self.left_nums -= count;
            return Ok(());
        }

        count -= self.left_nums;

        let ratio = (count as f32 * 1.0f32) / (self.rate.nums as f32) + 1.0f32;
        self.left_nums = 0;
        if self.left_nums == 0 {
            self.util += self.rate.per.mul_f32(ratio);
            self.sleep.as_mut().set(tokio::time::sleep_until(self.util));
        }
        return Ok(());
    }
}
