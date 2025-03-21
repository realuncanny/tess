<div align="center">
  <img width="2657" alt="overview" src="https://github.com/user-attachments/assets/10a9ed6a-ade2-45b9-bf9f-b10840a51fe1" />
</div>

### Some of the features:

- Customizable and savable grid layouts, selectable themes
- Supports most of the spot(USDT) and linear perp pairs from Binance & Bybit
- Tick size multipliers for price grouping on `FootprintChart` & `HeatmapChart`
- `CandlestickChart` & `FootprintChart` supports ticks(based on trade counts) to be used as intervals, alongside traditional time(timeseries) intervals

<div align="center">
  <img width="284" alt="layouts" src="https://github.com/user-attachments/assets/cca84a96-1fe5-4286-85e9-f4f26c5b1dc0" />
  <img width="245" alt="tickers-table" src="https://github.com/user-attachments/assets/51b84aef-ed45-4a73-90e8-e77d8ab80438" />
</div>

##### User receives market data directly from exchanges' public REST APIs & Websockets over TLS

- Orderbook total bid/ask levels: 1000 for Binance Perp/Spot; 500 for Bybit Perps, 200 for Bybit Spot
- Binance perp/spot & Bybit perp pairs streams @100ms; Bybit spot pairs streams @200ms
- As historical data, it can fetch OHLCV, open interest and partially, trades:

#### Historical trades on footprint chart:

Optionally, you can enable trade fetching from settings menu, experimental because of unreliability:

- Binance connector supports downloading historical trades from [data.binance.vision](https://data.binance.vision), fast and easy way to get trades, but they dont support intraday data.
Intraday trades fetched by pagination using Binance's public REST APIs: `/fapi/v1/aggTrades` & `api/v3/aggTrades`, which might be slow because of rate-limits

- Bybit itself doesnt have a similar purpose public REST API

Flowsurface can use those ends with Binance tickers to visualize historical public trades while being independent of a 'middleman' database between exchange and the user

So, when a chart instance signal the exchange connector after a data integrity check, about missing trades in the visible range; it tries via fetching, downloading and/or loading from cache, whichever suitable to get desired historical data

## Build from source

The releases might not be up-to-date with newest features.<sup>or bugs :)</sup>

- For that you could
  clone the repository into a directory of your choice and build with cargo.

Requirements:

- [Rust toolchain](https://www.rust-lang.org/tools/install)
- [Git version control system](https://git-scm.com/)

```bash
# Clone the repository
git clone https://github.com/akenshaw/flowsurface

cd flowsurface

# Build and run
cargo build --release
cargo run --release
```

<a href="https://github.com/iced-rs/iced">
  <img src="https://gist.githubusercontent.com/hecrj/ad7ecd38f6e47ff3688a38c79fd108f0/raw/74384875ecbad02ae2a926425e9bcafd0695bade/color.svg" width="130px">
</a>
