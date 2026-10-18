# Windows 移植（TSF）

> **未在 Windows 實機驗證。** 這裡的程式只在 Linux 上以 `x86_64-pc-windows-gnu` 交叉編譯過，沒有錯誤也沒有警告。host 的邏輯（`host/src/hub.rs`）在 Linux 上用真的 core 與資料測過（`host/tests/hub.rs`），協定有往返測試；TSF DLL、named pipe、候選窗的繪製、登錄與安裝腳本都還沒在 Windows 上跑過。

## 架構

```
App 程序                               shanjie-host.exe（每個使用者一個）
┌──────────────────────┐   call pipe   ┌───────────────────────────────┐
│ shanjie_tsf.dll       │ ────────────▶ │ hub：Shell（ports/shell）       │
│  讀：前文、密碼欄、游標 │ ◀──────────── │      core 引擎（一個）          │
│  寫：組字、提交        │   event pipe  │ 候選窗（GDI）                  │
│  語言列選單            │ ◀──────────── │ 語意校正背景執行緒             │
└──────────────────────┘               └───────────────────────────────┘
```

- `proto/`：雙方的訊息與按鍵分類（只用 std，任何平台都能測）。
- `host/`：`src/hub.rs` 是與平台無關的邏輯（可在 Linux 測）；`src/win/` 是訊息迴圈、pipe 伺服器與候選窗。
- `tsf/`：COM 進入點、註冊、文字服務、語言列按鈕、display attribute、與 host 的連線。DLL 不含 core。

每個 App 執行緒的 TSF 實例是 host 的一個 session（相當於 macOS 的一個 IMK client）。按鍵在同步 edit session 裡送給 host 並等回覆；別的來源的文字（候選窗點選、語意校正結果、在別的 App 按鍵造成的提交）經 event pipe 送回，DLL 用非同步 edit session 寫入。

## 建置與安裝

```bash
ports/windows/scripts/package.sh   # 產生 zip，內含 install.ps1、uninstall.ps1、README.txt
```

在 Windows 上：解壓縮，以系統管理員身分開 PowerShell：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

## 在 Windows 上要先驗證的事

1. `regsvr32` 註冊成功，設定 > 語言 裡出現「善解」，Win+Space 可以選到。
2. 記事本打 `su3cl3` 再按 Enter 得到「你好」；候選窗出現在游標下方，點選有效。
3. Discord（`discord.exe`）與 Word 分別打 row284，得到「率水壺」與「律水壺」。
4. 密碼欄不學習、失焦不提交；語言列選單的每個項目。
5. AppContainer App（設定、Microsoft Store 的 App）裡能連到 host。
6. host 不在時按鍵照常放行，App 不卡。
7. 開啟「中英文之間自動加空格」後，在記事本的英文後面打中文，開頭會有空白（DLL 要讀得到游標前的文字）。
