# 驗證：做過什麼、怎麼做的、還有什麼沒做

上游 `1d6547d` 為底。CI：`.github/workflows/ports.yml`（每次推到 `ports` 都跑）。

## 已驗證

| 範圍 | 怎麼驗證 | 在哪裡 |
| --- | --- | --- |
| 資料夾與上游 macOS app 的 Resources 相同 | `ports/data/fetch.sh` 依上游記錄的 SHA-256 下載與檢查；`assemble.sh` 照 `scripts/build-app.sh` 的清單組裝，產生的 english-words.txt 也比對上游的 SHA-256 | 本機、CI |
| shell（`ports/shell`）的行為 | 39 個測試，用真的 core 與資料，逐項移植自 ShellTests.swift、CJKSpacingTests.swift 等；中英文之間自動加空格的契約列出的 8 個突變，每一個都至少讓一個測試失敗 | 本機、CI |
| Linux 引擎（IBus） | 9 個端對端情境，在私有的 dbus 與 ibus-daemon 上，用 IBus 的 Python client 打字（`ports/linux/tests`）：你好、row284 書面與聊天、失焦只提交一次、密碼欄捨棄、預測列數字選字、數字鍵盤、選單開關存檔、中英之間的開頭空白 | 本機、CI |
| Linux 套件 | 打包後從安裝後的目錄結構執行過情境 | 本機 |
| Windows 程式能編譯 | `x86_64-pc-windows-gnu` 交叉編譯，警告視為錯誤；CI 另在 Windows runner 上用 MSVC 編譯 | 本機、CI |
| Windows host 的邏輯 | `host/tests/hub.rs`（8 個）用真的 core 與資料、假的 pipe 與候選窗；`proto` 的訊息往返與按鍵分類測試 | 本機（Linux）、CI（Linux 與 Windows） |
| core 的 Windows shim | host 測試在 Windows runner 上跑：trigram 表用 `MapViewOfFile` 映射；選字寫進 learning.tsv、新的引擎再打開它（目錄、檔案與日期的 shim） | CI |

## 未驗證

**Windows 的執行期全部沒有在實機上跑過**：

- TSF DLL：註冊、語言 profile、按鍵（OnTestKeyDown／OnKeyDown）、edit session、display attribute、語言列選單與圖示。
- host 與 DLL 之間的 named pipe（含 AppContainer 的存取權限）、host 自動啟動與重連。
- GDI 候選窗的位置、DPI、點選。
- `install.ps1`／`uninstall.ps1`。

要先檢查的項目列在 `ports/windows/README.md`。

Linux 方面沒有自動化的部分：

- 桌面上的候選窗外觀（GNOME Shell、KDE 等由桌面自己畫）。
- 各 App 回報的 client 名稱與前文（surrounding text）是否可靠；情境測試用的是 IBus 的測試 client。
- 「清除失敗」的 `notify-send` 通知。
