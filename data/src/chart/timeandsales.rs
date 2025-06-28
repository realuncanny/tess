use serde::{Deserialize, Serialize};

const DEFAULT_BUFFER_SIZE: usize = 900;

#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub struct Config {
    pub trade_size_filter: f32,
    #[serde(default = "default_buffer_filter")]
    pub buffer_filter: usize,
    pub stacked_bar_ratio: StackedBarRatio,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            trade_size_filter: 0.0,
            buffer_filter: DEFAULT_BUFFER_SIZE,
            stacked_bar_ratio: StackedBarRatio::default(),
        }
    }
}

fn default_buffer_filter() -> usize {
    DEFAULT_BUFFER_SIZE
}

pub struct TradeDisplay {
    pub time_str: String,
    pub price: f32,
    pub qty: f32,
    pub is_sell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default, Copy)]
pub enum StackedBarRatio {
    TotalVolume,
    #[default]
    Count,
    AverageSize,
    VolumeImbalance,
}

impl std::fmt::Display for StackedBarRatio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StackedBarRatio::TotalVolume => write!(f, "Total Volume"),
            StackedBarRatio::Count => write!(f, "Count"),
            StackedBarRatio::AverageSize => write!(f, "Average Size"),
            StackedBarRatio::VolumeImbalance => write!(f, "Volume Imbalance"),
        }
    }
}

impl StackedBarRatio {
    pub const ALL: [StackedBarRatio; 4] = [
        StackedBarRatio::TotalVolume,
        StackedBarRatio::Count,
        StackedBarRatio::AverageSize,
        StackedBarRatio::VolumeImbalance,
    ];

    pub fn calculate(&self, trades: &[TradeDisplay]) -> Option<(f32, f32)> {
        match self {
            StackedBarRatio::TotalVolume => {
                let (buy_volume, sell_volume) = trades.iter().fold((0.0, 0.0), |(buy, sell), t| {
                    if t.is_sell {
                        (buy, sell + t.qty)
                    } else {
                        (buy + t.qty, sell)
                    }
                });

                let total_volume = buy_volume + sell_volume;
                (total_volume > 0.0)
                    .then(|| (buy_volume / total_volume, sell_volume / total_volume))
            }
            StackedBarRatio::Count => {
                let (buy_count, sell_count) = trades.iter().fold((0, 0), |(buy, sell), t| {
                    if t.is_sell {
                        (buy, sell + 1)
                    } else {
                        (buy + 1, sell)
                    }
                });

                let total_count = buy_count + sell_count;
                (total_count > 0).then(|| {
                    (
                        buy_count as f32 / total_count as f32,
                        sell_count as f32 / total_count as f32,
                    )
                })
            }
            StackedBarRatio::AverageSize => {
                let (buy_volume, buy_count, sell_volume, sell_count) = trades.iter().fold(
                    (0.0, 0, 0.0, 0),
                    |(b_volume, b_count, s_volume, s_count), t| {
                        if t.is_sell {
                            (b_volume, b_count, s_volume + t.qty, s_count + 1)
                        } else {
                            (b_volume + t.qty, b_count + 1, s_volume, s_count)
                        }
                    },
                );

                let avg_buy_size = (buy_count > 0)
                    .then(|| buy_volume / buy_count as f32)
                    .unwrap_or(0.0);
                let avg_sell_size = (sell_count > 0)
                    .then(|| sell_volume / sell_count as f32)
                    .unwrap_or(0.0);

                let total_avg_size = avg_buy_size + avg_sell_size;
                (total_avg_size > 0.0).then(|| {
                    (
                        avg_buy_size / total_avg_size,
                        avg_sell_size / total_avg_size,
                    )
                })
            }
            StackedBarRatio::VolumeImbalance => {
                let (buy_volume, sell_volume) = trades.iter().fold((0.0, 0.0), |(buy, sell), t| {
                    if t.is_sell {
                        (buy, sell + t.qty)
                    } else {
                        (buy + t.qty, sell)
                    }
                });

                let total_volume = buy_volume + sell_volume;
                (total_volume > 0.0).then(|| {
                    let volume_imbalance = (buy_volume - sell_volume) / total_volume;
                    let buy_ratio = (1.0 + volume_imbalance) / 2.0;
                    (buy_ratio, 1.0 - buy_ratio)
                })
            }
        }
    }
}
