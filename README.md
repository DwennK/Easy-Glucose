<p align="center"><img src="assets/icon.png" alt="" width="86"></p>
<h1 align="center">Easy Glucose</h1>
<p align="center">View your glucose in the menu bar, in a floating widget, or on a 24-hour chart.</p>
<p align="center"><a href=".github/workflows/build.yml"><img src="https://github.com/DwennK/Easy-Glucose/actions/workflows/build.yml/badge.svg" alt="Native builds and tests"></a> · Rust · macOS and Windows · <a href="LICENSE">MIT License</a></p>

<p align="center"><a href="#try-the-demo">Try the demo</a> · <a href="#three-views">Explore the views</a> · <a href="#connect-a-data-source">Connect a data source</a> · <a href="#build-from-source">Build from source</a></p>

![Native dashboard showing six hours of simulated glucose readings in mmol/L](docs/images/dashboard.png)

## Try the demo

With stable Rust and your platform’s build tools installed, run the following commands. The demo uses simulated readings and requires no account. The screenshots in this README show the demo running on macOS.

```sh
git clone https://github.com/DwennK/Easy-Glucose.git
cd Easy-Glucose
cargo run --release --locked -- --demo
```

To view your own readings, run `cargo run --release --locked` and open **Settings → Data sources**. Easy Glucose shows how old each reading is. Use it as a secondary display and rely on your sensor’s official app for alarms and treatment decisions.

## Three views

| macOS menu bar preview | Floating widget |
| --- | --- |
| <img src="docs/images/menu-preview.png" width="420" alt="Compact glucose preview with two action buttons and a close button in the upper right"> | <img src="docs/images/widget.png" width="320" alt="Floating widget showing the glucose value, trend, reading time, and chart"> |
| Click the glucose value to open the chart. The two buttons next to the close button open the dashboard and settings. Press Esc or click outside the preview to close it. | Drag the widget by its header. It remembers its position. Choose from three sizes and turn on **Always on top** to keep your reading visible over other windows. |

In **My glucose**, choose a time range of 1, 3, 6, 12, or 24 hours. Each point represents a reading, and gaps in the data remain visible. Hover over the chart to see the value and time of the nearest reading. Readings at least ten minutes old are marked as stale, even when the connection is working.

Right-click the macOS menu bar item or open the Windows system tray menu to access the main window, settings, and floating widget, or to quit the app.

## Connect a data source

| Source | What you need |
| --- | --- |
| **Dexcom Share** | Use the sensor wearer’s Dexcom account and select its region. Enable Share in Dexcom and add at least one follower. |
| **FreeStyle Libre · LibreLinkUp** | Use a LibreLinkUp account that has accepted a sharing invitation. If several people share data with you, choose whose readings to display. |
| **Nightscout / Nocturne** | Enter the site’s HTTPS URL. For a private site, provide a read token. Public sites work without a token. API secret authentication is also available. |

Credentials are stored in macOS Keychain or Windows Credential Manager. Easy Glucose keeps a separate local history for each source and associates it with the relevant account, person, or site. The app does not encrypt the local readings file. The Dexcom Share and LibreLinkUp connections use unofficial protocols. [How the connectors work and their limitations (in French)](docs/sources.md)

## Languages and units

The interface uses your system language by default. You can change it on the welcome screen or under **Settings → Display → Language** without restarting. You can display readings in **mmol/L** or **mg/dL** in any language.

**20 languages** — Български, Čeština, Dansk, Deutsch, Ελληνικά, English, Español, Français, Italiano, Magyar, Nederlands, Norsk bokmål, Polski, Português, Română, Slovenčina, Suomi, Svenska, Türkçe, Українська.

## Build from source

On macOS, install the Xcode Command Line Tools. On Windows, install Visual Studio Build Tools with C++ and the Windows SDK. Python 3 is only required for packaging.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/licenses.py
python3 scripts/package.py
```

On macOS, open `dist/Easy Glucose.app`. On Windows, run the two packaging scripts with `python`, then open `dist/Easy-Glucose-Windows/Easy Glucose.exe`. CI also produces archives for Apple Silicon Macs, Intel Macs, and Windows x64. The macOS bundles are ad hoc signed and have not been notarized by Apple. The Windows executable is unsigned. [Build and distribution guide (in French)](docs/distribution.md)

## Code layout

| Path | Purpose |
| --- | --- |
| [src/core.rs](src/core.rs) | Unit conversions, glucose trends, reading age, and reading selection. |
| [src/sources](src/sources) · [src/dexcom.rs](src/dexcom.rs) | Provider clients, authentication, and reading retrieval. |
| [src/services.rs](src/services.rs) · [src/storage.rs](src/storage.rs) | Background synchronization, caching, and persistence. |
| [src/ui](src/ui) · [src/platform](src/platform) | Native egui interface, floating widget, and menu bar integration. |
| [locales](locales) | One catalog per language. The build checks that every message has a translation. |

Provider tests use local servers and simulated data. [Architecture (in French)](docs/architecture.md) · [Validation (in French)](docs/validation.md) · [Third-party licenses](THIRD_PARTY.md)
