# 移植決定：哪裡和 macOS 版不同、為什麼

原則：行為照上游 macOS 版。只有在平台沒有對應的東西，或平台的做法不一樣時，才換成最接近的做法。下面列出每一個這樣的地方。上游沒有的「Windows 操作」（舊移植版的設定）另列在 `windows-habits.md`，預設全部停在上游行為。除此之外，兩個移植版的按鍵處理、選單、設定、學習閘門、聊天／書面、組字擁有者與語意校正佇列都是 `ports/shell`，由 Shell.swift、Learning.swift 逐段移植，並有對應的測試（`ports/shell/tests/shell.rs`）。

## 共同

| macOS | 移植版 | 原因 |
| --- | --- | --- |
| 一個輸入法程序擁有 shell 與引擎，所有 App 都是它的客戶（IMK） | Linux：ibus-engine-shanjie 一個程序。Windows：shanjie-host.exe 一個程序，每個 App 裡的 TSF DLL 只負責讀寫文件 | 只有單一程序，才能照上游的組字擁有者、單一引擎與語意校正佇列運作 |
| 設定存在 UserDefaults | `settings.ini`，鍵名與 UserDefaults 相同 | 平台沒有 UserDefaults |
| 設定視窗（善解設定…） | 選單中「善解設定…」的位置換成視窗裡的設定，每項一個子選單：切換方式、個別設定的 App（點選即移除）、英文的顯示、語意校正強度、候選窗方向；「中英文之間自動加空格」是一個開關，它下面那行說明在選單裡沒有位置，不顯示。鍵盤排列與其他開關原本就在選單裡 | 先不另做設定程式；IBus 面板與 TSF 語言列都只支援一層子選單 |
| 字元的單位（中英文之間自動加空格） | Swift 逐個 `Character`（字素）判斷；移植版沒有字素切分，把變體選擇符、組合附加符號、零寬連接符併進前一個字，其他字元本來就不觸發，結果相同 | Rust 標準庫沒有字素切分，不為此加相依套件 |
| 玻璃色調滑桿 | 不顯示，值照樣儲存 | 移植版的候選窗沒有毛玻璃 |
| 清除選字記憶的確認對話框（清除／取消） | 選單「清除選字記憶…」變成子選單：標題、清除、取消 | 和設定一樣放進選單，免開視窗 |
| 清除失敗的對話框 | Linux：`notify-send` 通知。Windows：MessageBox（按鈕是系統的「確定」，上游是「好」） | 平台的對話框 |
| 「不要備份選字記憶」（Time Machine） | 不提供 | 沒有 Time Machine |
| 自動更新（Sparkle） | 不提供 | 移植版沒有更新程式 |
| 系統標點表、標點名稱（s3e、s3f） | 不提供；core 用內建的標點表 | 平台沒有對應的系統資料 |
| 修飾鍵：Option、Command | Alt 當 Option，Windows 鍵／Super 當 Command | 位置相同的鍵 |
| 按鍵：只看實體位置（kVK 碼） | Linux 看 evdev 碼，Windows 看 scan code，對應同樣的 ANSI 鍵位 | 鍵盤配置不影響送進 core 的鍵 |
| 單獨按修飾鍵是 flagsChanged，shell 不處理 | 單獨的修飾鍵（含 Caps Lock、Num Lock、Scroll Lock）直接放行 | 同上 |
| App 識別：bundle ID | Linux：IBus client 名稱冒號後面的程式名（`gtk3-im:gedit` → `gedit`）。Windows：執行檔名，小寫（`discord.exe`） | 平台能拿到的識別 |
| 聊天 App 清單、學習排除清單 | 換成同一個 App 在該平台的識別，沒有該平台版本的 App 就沒有對應項（`ports/linux/src/tables.rs`、`ports/windows/host/src/tables.rs`） | 同上 |
| 安全輸入（IsSecureEventInputEnabled） | Linux：輸入欄位的 purpose 是 PASSWORD 或 PIN。Windows：input scope 是密碼或 PIN | 平台的密碼欄位標示 |
| 候選窗跟著游標 | Linux：由桌面（GNOME Shell 等）畫。Windows：host 自己畫在 DLL 回報的游標位置下方 | 見各平台 |
| 選字記憶位置 | Linux `~/.local/share/shanjie`，Windows `%LOCALAPPDATA%\Shanjie`；兩處都和舊移植版相同，learning.tsv 格式相同，可以沿用 | — |

## Linux（IBus）

| 項目 | 做法 | 原因 |
| --- | --- | --- |
| 組字的提交 | preedit 用 COMMIT 模式：失去焦點或 reset 時由 client 提交畫面上的組字，shell 那次的 `Insert` 不再重送。密碼欄位用 CLEAR 模式 | 與 macOS 失焦提交相同的結果，而且不會重複提交 |
| 英文段的粗底線 | 雙底線 | IBus 沒有粗底線 |
| 展開格狀候選 | 只顯示選取的那一列（號碼照上游規則） | IBus 候選窗不能畫格狀 |
| 候選編號 | 每格都給標籤，不顯示號碼的格給空白 | 少了標籤，GNOME Shell 會自己補上預設號碼 |
| 英文段提示（空白：英文斷開） | IBus 的 auxiliary text | IBus 沒有單獨的提示窗 |
| 選單 | IBus properties：鍵盤排列是 RADIO，開關是 TOGGLE，資訊行是不可選的 NORMAL | IBus 的選單元件 |
| App 識別 | 引擎宣告 FocusId，daemon 才會用 FocusInId 傳 client 名稱。IBus 1.5.32 以前（例如 Ubuntu 24.04 的 1.5.29），引擎啟動後的第一次聚焦在 daemon 讀到這個屬性之前就送出，沒有 client 名稱：那個 App 要等焦點換過一次才認得（之前當成未知的 App：書面、學習暫停）。引擎這邊拿不到名稱，無法補救。GNOME Wayland 下有些 App 可能只回報 `gnome-shell`，這時當成一個已知 App（用書面） | IBus 的介面 |
| 左側前文 | 宣告 ActiveSurroundingText，讀 SetSurroundingText；不支援的 App 就沒有前文 | IBus 的介面 |

## Windows（TSF）

全部**未在 Windows 實機驗證**，見 `ports/windows/README.md`。

| 項目 | 做法 | 原因 |
| --- | --- | --- |
| 程序架構 | TSF DLL 只讀寫文件；shell、引擎與候選窗在 shanjie-host.exe（每個使用者一個），用 named pipe 溝通 | 照 macOS 的單一程序模型 |
| 哪些按鍵送給 host | OnTestKeyDown 接下所有按鍵，只有單獨的修飾鍵，以及沒有組字時的 Ctrl／Alt／Windows 鍵組合除外（Ctrl+\ 照樣接，core 會用）。OnKeyDown 在同步 edit session 裡問 host，回傳 shell 的答案 | 上游 shell 自己決定處理或放行；這樣放行的鍵仍會到 App。代價：沒有組字時，在別的 App 按 Ctrl 組合鍵不會先提交擁有者的組字 |
| host 沒回應 | 每次呼叫最多等 800 ms，逾時就斷線、放行按鍵，5 秒後才重連 | 不讓 App 卡住 |
| 組字底線 | display attribute：一般底線；英文段粗底線 | TSF 的組字樣式 |
| 中／英開關 | 啟用時把 OPENCLOSE 設為開；App 或系統把它關掉時，按鍵全部放行 | 上游沒有中英切換（macOS 是切換輸入來源）；Windows 用 Win+Space 切換輸入法 |
| 鍵盤排列 | 一個輸入法 profile，標準／倚天在選單裡切換 | 與 macOS 相同；舊版是兩個 profile |
| 語言列圖示 | 執行時用 GDI 畫一個「善」 | 套件裡還沒有圖示資源 |
| 候選窗 | GDI 畫的置頂視窗，不搶焦點；系統配色與 UI 字型；橫排、直排、展開格狀都畫 | 平台的視窗 |
| 安裝 | `%ProgramFiles%\ShanjiePorts\<版本>`，每個版本一個資料夾；登入時啟動 host | 使用中的 DLL 與映射中的模型檔不能被覆寫 |
| 識別碼 | 新的 CLSID 與 profile GUID | 與舊版可以並存，不會互相覆蓋 |
