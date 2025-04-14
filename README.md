<div align="center">
  <img width="2554" alt="overview-layout" src="https://github.com/user-attachments/assets/50fc7fe5-2bd6-4d6f-8040-19e9fd194e25" />
</div>

### Some of the features:

- Customizable and savable grid layouts, selectable themes
- Supports most of the spot(USDT) and linear/inverse perp pairs from Binance & Bybit
- Tick size multipliers for price grouping on `FootprintChart` & `HeatmapChart`
- Alongside traditional Time-based charts, `CandlestickChart` & `FootprintChart` supports "ticks"(based on aggregated trade streams) to be used as intervals to create Tick-based charts

<div align="center">
  <img width="283" alt="layout-manager" src="https://github.com/user-attachments/assets/6ffac895-5f8c-4d9e-a1e8-a8bd41bd7fc3" />
  <img width="242" alt="expanded-ticker-card" src="https://github.com/user-attachments/assets/c75161bc-e572-4737-a315-115545e27bbe" />
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

Requirements:

- [Rust toolchain](https://www.rust-lang.org/tools/install)
- [Git version control system](https://git-scm.com/)
- System dependencies:
  - **Linux**:
    - Debian/Ubuntu: `sudo apt install build-essential pkg-config libasound2-dev`
    - Arch: `sudo pacman -S base-devel alsa-lib`
    - Fedora: `sudo dnf install gcc make alsa-lib-devel`
  - **macOS**: Install Xcode Command Line Tools: `xcode-select --install`
  - **Windows**: No additional dependencies required

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
