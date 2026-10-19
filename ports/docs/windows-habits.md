# Windows 操作：舊移植版的設定與 Windows 使用者的習慣

這份文件說明移植版加上的「Windows 操作」：舊移植版（shanjie-multiplatform 0.10.2，`profiles/windows.ini`）的設定，以及從 Windows（微軟注音）換過來的人會找的按鍵。上游沒有這些功能。新安裝時每一項都關著，或停在上游原本的行為（例如「Caps Lock 打英文」預設開，就是上游的行為），所以不選就和上游完全一樣。

程式在 `ports/shell/src/winops.rs`（設定表與純函式）、`ports/shell/src/shell/win.rs`（按鍵處理）；測試在 `ports/shell/tests/windows_habits.rs`。原則和移植版其他部分相同：**不改 `core/`**。每一項都在 shell 把按鍵交給核心之前處理：改寫按鍵、攔下按鍵，或改 shell 送出的文字；核心收到的仍然只有它原本就收的按鍵。

## 在哪裡設定

移植版沒有設定視窗，設定都在輸入法選單裡（`ports/docs/porting.md`）。「善解設定…」的位置後面多兩個子選單：

- **Windows 操作**：除了標點以外的全部，分成切換、按鍵、中英混打、學習、範本幾組，每組前面有一行標題。
- **Windows 操作：標點**：19 個標點各一個開關，以及「全部打中文標點」「全部改成直接送出按鍵」。

IBus 面板與 TSF 語言列都只支援一層子選單，所以分成兩個，不再往下分。

「範本」一組有兩個項目：

- **套用舊版 Windows 操作**：照舊版 `windows.ini` 設定每一項，並且照那份範本改三個上游設定：關閉即時預測、關閉上游中英混打、改成一律聊天。
- **Windows 操作全部還原成上游**：只還原 Windows 操作，上游設定不動。

設定存在 `settings.ini`，鍵名是 `win.` 開頭（例如 `win.shiftToggle=true`）。打包時也可以用 overlay 改從沒選過的預設值（`ports/overlay/`，鍵的清單在 `merge.sh` 的 `known`）。

## 對照：舊版設定與新版

| 舊版（`windows.ini`） | 新版 | 鍵 |
| --- | --- | --- |
| `shortcuts.shift_toggle`、`shift_side` | 單按 Shift 切換中／英；左右 Shift 都可以／只用左／只用右 | `win.shiftToggle`、`win.shiftSide` |
| `shortcuts.ctrl_space_toggle` | Ctrl＋空白 切換中／英 | `win.ctrlSpaceToggle` |
| `shortcuts.shift_space_toggle`、`input.full_width` | Shift＋空白 切換全形／半形；「全形」也可以直接勾 | `win.shiftSpaceWidth`、`win.fullWidth` |
| `input.chinese` | 「中文輸入」：目前的中／英狀態，不存檔，每次啟動都是中文 | — |
| `input.caps_lock_english` | Caps Lock 打英文（開是上游行為；關掉時 Caps Lock 開著也照樣打注音） | `win.capsLockEnglish` |
| `shortcuts.ctrl_punctuation` | Ctrl＋標點 打中文標點 | `win.ctrlPunctuation` |
| `shortcuts.symbol_keyboard` | Ctrl＋Alt＋逗號 開符號表 | `win.symbolTable` |
| `shortcuts.code_input` | 反引號＋U 輸入 Unicode 字碼 | `win.codeInput` |
| `candidates.enter_commits` | 選字時 Enter 直接送出 | `win.enterCommits` |
| `input.learning` | 學習選字 | `win.learning` |
| `input.context` | 看游標前的字選字 | `win.leftContext` |
| `input.mixed_input`（Windows 注音順序） | Windows 注音順序混打 | `win.mixedInput` |
| `input.mixed_tolerance` | 混打：容許多按一次同一鍵 | `win.mixedTolerance` |
| `[punctuation]` 19 個開關 | Windows 操作：標點 | `win.punct.shift.*`、`win.punct.plain.*` |
| `engine.prediction`、`engine.ranking`、`input.upstream_english` | 上游原本就有的設定（即時預測、切換方式、中英混打） | 上游的鍵 |
| `candidates.space_pages`、`wrap_pages` | 上游原本就是：候選窗開著時空白翻頁，最後一頁回第一頁 | — |
| `input.escape=staged` | 上游原本就是：Esc 先清沒打完的注音，再清整句 | — |

## 每一項做了什麼

**中／英**：切到英文時，正在打的字先送出，之後每個鍵都直接給 App。單按 Shift 是在放開時才切換，而且中間不能按別的鍵、不能同時按 Ctrl 等修飾鍵、不能兩個 Shift 一起按；按住 Shift 時的自動重複不算。平台要把單獨的修飾鍵告訴 shell（`Shell::modifier`）：

- Linux：IBus 送來的 Shift_L／Shift_R 按下與放開。
- Windows：TSF DLL 在 OnTestKeyDown 送按下、在 OnKeyUp 送放開，通訊協定新增 `Request::Modifier`，版本升為 2。修飾鍵本身仍然交給 App。

**全形**：shell 送出的 ASCII 字元換成全形（`A` → `Ａ`），空白換成全形空白。英文模式下打的字由 shell 直接送出全形字；中文模式下沒有組字時，空白鍵打出全形空白。

**Caps Lock 打英文**：關掉時，shell 把 Caps Lock 的狀態拿掉再交給核心，核心就照常打注音。

**Ctrl＋標點**：只換成核心本來就有的標點：Ctrl＋, . ; 變成 ，。：；Ctrl＋[ ] 變成 「」；Ctrl＋Shift＋標點鍵和 Shift＋標點鍵一樣。舊版有些對照是核心標點表裡沒有的符號：Ctrl＋; 舊版是「；」，這裡是核心有的「：」；Ctrl＋/、Ctrl＋' 沒有做。

**標點開關**：關掉的標點，正在打的字先送出，按鍵直接給 App（例如 Shift＋, 打出 `<`）。開關只管核心會變成標點的鍵；倚天鍵盤的 `'`、`=` 是注音鍵，不受影響。

**符號表**：Ctrl＋Alt＋逗號 打開舊版那份符號表，每頁 9 個：數字或 Enter 選、方向鍵移動、空白換頁、Esc 或 Backspace 關閉。打開前，正在打的字先送出。

**字碼輸入**：沒有組字時按反引號，再按 U 和最多 6 位十六進位數字，空白或 Enter 送出那個字。反引號後面接的不是 U 時，反引號照上游打出「·」，再處理那個鍵。舊版的 Big5 字碼（反引號＋B）要用系統的 950 碼頁表，沒有做。

**選字時 Enter 直接送出**：上游在候選窗裡按 Enter 只確認選字、繼續組字，這個選項打開時會接著把整句送出。

**學習選字、看游標前的字**：關掉時，學習閘門一律暫停（和上游在密碼欄位的做法相同）；前文不送進核心。

## Windows 注音順序混打

規則照舊版 `platform/smart_input_windows.rs` 移植，搬到 shell 執行：

- 沒打完的注音照按鍵顯示（`gj`），下方提示讀音（ㄕㄨ）；按聲調或空白才轉成中文（`su3` → 你）。
- 以英文字母開頭時，要照聲母、介音、韻母的順序，而且每一欄只能出現一次；不符合就當英文（`hello`）。以數字或符號開頭時，順序不拘（`0d4` → 看）。
- 英文後面接聲調時，最後幾個鍵如果組得成字，就拆成英文加中文（`callsu3` → call你）。空白之前是英文詞表裡的詞時，空白留在英文後面（`hello world`）。
- Shift＋字母直接開始英文，大小寫照打的保留。Backspace 一次刪一個鍵，Esc 清掉，↓／Tab 改看注音候選，Enter 照打的送出。
- 數字開頭而組不成字時，後面的數字鍵都當數字（`13`）。
- 「容許多按一次同一鍵」：2 到 4 個鍵裡有一個相鄰重複，而且去掉之後剛好只有一種字，就用那個字（`ccl3` → 好）。英文詞表裡的詞不受影響。

資料都用上游的：讀音能不能成字，看上游詞庫 `mcbpmf-data.txt` 的單字讀音；英文詞看上游的 `english-words.txt`（舊版用的是另一份 Rime 詞表）。英文用上游的英文段（英文 token）放進組字區，所以會和中文一起留在組字區、一起送出。為了能放英文段，這個選項打開時，shell 會打開核心的中英混打。如果上游的「中英混打」沒開，核心「打字時就顯示英文」那一項仍然維持關閉，兩邊不會把同一串鍵顯示兩次。

和舊版不同的地方：

- **只支援標準鍵盤**：上游的英文段只在標準鍵盤有，倚天鍵盤時這個選項不能選。
- **英文裡有數字或符號時，會連同整句先送出**：例如 `13su3` 會先送出「13」，再開始組「你」；`hello,` 會連同前面的字一起送出。核心的英文段只能放英文字母，舊版是在核心裡另外存字面文字。
- **容錯轉成字之後，不能再從候選選回原本打的鍵**：舊版的 `typed_literals` 要改核心。
- **上游中英混打也開著時（兩者並用）**：Shift＋字母交給上游的英文段，其他照上面的規則。

## 沒有做的（要改核心，或是另一個程式）

| 舊版功能 | 原因 |
| --- | --- |
| Enter 只送出游標前（`enter_to_cursor`） | 核心只有整句送出（`reset`），沒有送出一部分的呼叫 |
| 候選範圍在游標右側（`anchor=right`） | 候選是哪一段由核心決定 |
| 沒開候選時 ↑ 循環換字（`up_cycles`）、Backspace 刪候選（`backspace_deletes`） | 核心沒有對應的操作 |
| 開候選只用 ↓（`open=down`）、空白提交（`space_commits`）、提交後額外空白 | 空白在核心裡同時是一聲、開候選與翻頁，shell 攔下會打不出一聲 |
| Big5 過濾（`big5`）、選字後聯想（`associations`） | 要過濾或產生核心的候選 |
| 拼音鍵盤 | 要新的讀音資料（舊版 `pinyin-map.tsv`）與核心的轉換 |
| 獨立聲調鍵關閉（`standalone_tones`） | 上游現在空組字區的聲調鍵是數字混打的一部分，不是聲調符號 |
| 個人詞庫、雙 Ctrl 加詞、線上模型 | 獨立的功能，要另外規劃 |
| 設定視窗、候選字級與主題、中／英狀態提示 | 移植版目前沒有設定視窗；中／英狀態在選單第一項 |

## 驗證

- `ports/shell/tests/windows_habits.rs`：28 個測試，用真的核心與資料。混打的例子取自舊版的 `windows_smart_tests`。
- `ports/linux/tests/scenarios/windows-shift-tap-and-mixed.json`：Linux 端對端情境，用私有的 ibus-daemon 送真的 Shift_L／Shift_R 按下與放開，驗證單按 Shift 切換後打字直接給 App，再切回來打混打。
- Windows：協定往返測試與 host 的按鍵對照有測；TSF DLL 的 Shift 按下與放開**沒有在 Windows 實機跑過**（`ports/docs/verification.md`）。
