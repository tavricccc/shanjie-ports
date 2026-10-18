# Linux 移植（IBus）

`ibus-engine-shanjie`：一個程序，擁有 shell 與 core 引擎（`src/hub.rs` 的專用執行緒），對 IBus daemon 提供 Factory／Engine／Service 物件（`src/bus.rs`，zbus）。語意校正在另一個背景執行緒跑，結果送回 shell 執行緒。

- `src/keys.rs`：IBus 按鍵轉 shell 按鍵，只看 evdev 實體位置。
- `src/props.rs`：選單樹（`shanjie_shell::menu_tree`）轉 IBus properties。
- `src/tables.rs`：聊天 App 與學習排除清單的 Linux 識別。
- `tests/`：私有 dbus／ibus-daemon 上的端對端情境（`run-scenarios.sh`），用真的資料夾。`SHANJIE_TRACE=1` 會把 shell 執行緒收到的每個呼叫印到 stderr。

建置、測試與打包見 `ports/README.md`。安裝：解開 release 壓縮檔後執行 `./install.sh`（會取代舊移植版並移除它的設定程式）。
