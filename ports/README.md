# 善解的 Linux 與 Windows 移植

這個 repo 是 [Nanako0129/shanjie](https://github.com/Nanako0129/shanjie) 的 fork，只加移植層，不改上游行為。

- `main` 跟上游 main 一致，不放自己的 commit。
- `ports` 是 `main` 再加上 `ports/`、`core/` 的 Windows 作業系統 shim（`docs/core-windows.md`），沒有別的改動。更新上游的方法：先把 `main` 快轉到上游，再把 `main` 合併進 `ports`。

## 原則

1. **行為照上游**：shell 由 macOS 的 Shell.swift、Learning.swift 移植，資料夾內容與 macOS app 的 Resources 相同。只有平台做不到的地方才不同，每一項都列在 `docs/porting.md`。
2. **兩個平台完全分開建置與打包**：`linux/` 與 `windows/` 各是獨立的 Cargo workspace，各有自己的打包腳本，只共用 `shell/`、`data/`、`overlay/` 與上游的 `core/`。
3. **Windows 操作**：舊移植版的設定與 Windows 使用者的習慣（單按 Shift 切換中英、全形、標點開關、Windows 注音順序混打等），放在 shell，不改 `core/`，預設全部停在上游行為，見 `docs/windows-habits.md`。
4. **自己加的設定走 overlay**：`overlay/defaults.ini`（共用）加 `overlay/linux.ini` 或 `overlay/windows.ini`，打包時由 `overlay/merge.sh` 合併成資料夾裡的 `defaults.ini`。這只改使用者從沒選過的設定的預設值，存過的值永遠優先。三份目前都是空的，也就是完全照上游。Windows 操作的 `win.*` 鍵也可以寫在這裡。

## 目錄

| 路徑 | 內容 |
| --- | --- |
| `shell/` | 平台無關的 shell：按鍵、選單、設定、學習閘門、聊天／書面、組字擁有者、語意校正佇列（crate `shanjie-shell`） |
| `data/` | `fetch.sh` 下載上游 release 的模型檔並核對雜湊；`assemble.sh` 組出與 macOS Resources 相同的資料夾 |
| `overlay/` | 預設值 overlay 與合併腳本 |
| `linux/` | IBus 引擎、端對端測試（私有 ibus-daemon）、打包 |
| `windows/` | TSF DLL、host、協定、打包。**未在 Windows 實機驗證**（`windows/README.md`） |
| `docs/inventory.md` | 舊移植版與上游的行為差異盤點 |
| `docs/porting.md` | 移植版和 macOS 版不同的地方，以及原因 |
| `docs/windows-habits.md` | Windows 操作：舊版設定對照、每一項做了什麼、沒做的與原因 |
| `docs/core-windows.md` | `core/` 的 Windows 修補 |
| `docs/verification.md` | 驗證了什麼、怎麼驗證、還有什麼沒驗證 |

## 建置

```bash
ports/data/fetch.sh                        # 一次就好：模型檔與英文詞表的來源
ports/data/assemble.sh build/ports/data    # 測試用的資料夾

# shell
(cd ports/shell && cargo test --release)

# Linux
(cd ports/linux && cargo test --release && tests/run-scenarios.sh)
ports/linux/scripts/package.sh             # build/ports/dist/shanjie-linux-<版本>-x86_64.tar.gz

# Windows（在 Linux 上交叉編譯；需要 x86_64-pc-windows-gnu 與 mingw-w64）
(cd ports/windows && cargo test --release) # 協定與 host 邏輯，用真資料
ports/windows/scripts/package.sh           # build/ports/dist/shanjie-windows-<版本>-x64.zip
```

版本號由 `version.sh` 產生，例如 `0.4.7+255.ports.8b14f81`：上游 release、之後的上游 commit 數、ports 的 commit。
