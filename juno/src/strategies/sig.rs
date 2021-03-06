use super::{Signal, SignalParams, SignalParamsContext, Strategy, StrategyMeta};
use crate::{
    genetics::Chromosome,
    utils::{combine, BufferedCandle, MidTrend, MidTrendPolicy, MidTrendPolicyExt, Persistence},
    Advice, Candle, Interval,
};
use juno_derive::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};

#[derive(Chromosome, Clone, Copy, Debug, Deserialize, Serialize)]
pub struct SigParams {
    #[chromosome]
    pub sig: SignalParams,
    #[serde(default)]
    pub persistence: u32,
    pub mid_trend_policy: MidTrendPolicy,
    #[serde(default)]
    pub buffer_interval: Option<Interval>,
}

fn persistence(rng: &mut StdRng) -> u32 {
    rng.gen_range(0..10)
}
fn mid_trend_policy(rng: &mut StdRng) -> MidTrendPolicy {
    rng.gen_mid_trend_policy()
}
fn buffer_interval(_rng: &mut StdRng) -> Option<Interval> {
    None
}

#[derive(Signal)]
pub struct Sig {
    sig: Box<dyn Signal>,
    mid_trend: MidTrend,
    persistence: Persistence,
    buffered_candle: BufferedCandle,
    advice: Advice,
    t: u32,
    t1: u32,
}

impl Sig {
    pub fn new(params: &SigParams, meta: &StrategyMeta) -> Self {
        let sig = params.sig.construct(meta);
        let mid_trend = MidTrend::new(params.mid_trend_policy);
        let persistence = Persistence::new(params.persistence, false);
        Self {
            advice: Advice::None,
            t: 0,
            t1: sig.maturity() + max(mid_trend.maturity(), persistence.maturity()) - 1,
            sig,
            mid_trend,
            persistence,
            buffered_candle: BufferedCandle::new(meta.interval, params.buffer_interval),
        }
    }
}

impl Strategy for Sig {
    fn maturity(&self) -> u32 {
        self.t1
    }

    fn mature(&self) -> bool {
        self.t >= self.t1
    }

    fn update(&mut self, candle: &Candle) {
        if let Some(candle) = self.buffered_candle.buffer(candle) {
            self.t = min(self.t + 1, self.t1);

            self.sig.update(candle.as_ref());
            if self.sig.mature() {
                self.advice = combine(
                    self.mid_trend.update(self.sig.advice()),
                    self.persistence.update(self.sig.advice()),
                );
            }
        }
    }
}
