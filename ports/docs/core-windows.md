# core 的 Windows 編譯修補

上游 core 只在 macOS（出貨）與 x86_64 Linux（測試）編譯。Windows 少的只有作業系統呼叫，沒有任何演算法差異。這份文件列出 fork 對 `core/` 的全部修改；除了這裡列的，`core/` 與上游逐位元相同。

| 檔案 | 修改 | 對行為的影響 |
| --- | --- | --- |
| `core/src/win_compat.rs`（新增） | Windows 才編譯。提供 learn_store.rs 用到的 Unix 擴充 trait（同名）、`GetLocalTime` 的本地日期、`CreateFileMappingW`／`MapViewOfFile` 的唯讀映射 | 無：只在 Windows 編譯 |
| `core/src/lib.rs` | `#[cfg(windows)] mod win_compat;` | 無 |
| `core/src/learn_store.rs` | 兩行 `use` 加 `#[cfg(unix)]`；Windows 改 import `win_compat` 的同名 trait 與常數；`extern "C"` 區塊加 `#[cfg(unix)]`；Windows 的 `barrier` 用 `sync_data`（FlushFileBuffers） | macOS／Linux 無（只加 cfg） |
| `core/src/trigram.rs` | mmap 相關常數與 `extern` 加 `#[cfg(unix)]`；新增 Windows 的 `Mapped::open`（同樣的檢查，換成 Windows 映射）與 `Drop` | macOS／Linux 無 |
| `core/src/learn.rs` | `local_day` 原實作加 `#[cfg(unix)]`；新增 Windows 版呼叫 `win_compat::local_day` | macOS／Linux 無 |

## Windows 與 Unix 的語意差距

- **學習檔權限**：Unix 版檢查目錄擁有者是本人、模式 0700，檔案 0600、只有一個 hard link。Windows 沒有對應的 POSIX 模式；學習目錄放在 `%LOCALAPPDATA%\Shanjie`，Windows 預設只有該使用者（與 SYSTEM、Administrators）能存取。shim 讓這些檢查一律通過，因此 Windows 上**沒有**「目錄被別人擁有就拒絕」這一層防護。
- **符號連結**：`O_NOFOLLOW` 對應 `FILE_FLAG_OPEN_REPARSE_POINT`，打開的是連結本身，`is_file` 判斷為否，學習檔會被當成損壞並重新開始，與 Unix 的結果相同（Unix 是 ELOOP 後同樣回傳 `Corrupt`）。
- **FIFO**：Windows 不能在一般目錄建立 FIFO，`O_NONBLOCK` 沒有作用。
- **寫入順序**：macOS 用 `F_BARRIERFSYNC`，Linux 測試用 `fdatasync`，Windows 用 `FlushFileBuffers`，之後同樣 rename。
- **trigram 映射**：Windows 在檔案被映射期間不允許取代或刪除該檔，因此安裝程式以新版本目錄取代資料，不覆寫使用中的檔案（見 `ports/windows/README.md`）。

## 驗證

- `cargo build --release -p core --target x86_64-pc-windows-gnu`：無錯誤、無警告（2026-10-19，本機交叉編譯）。
- Linux 上 `cargo build --release -p core` 與上游相同（上游本來就有兩個 learn_store.rs 的 unused 警告）。
- **Windows 實機執行未驗證。**
