# 行為差異盤點：舊移植版（shanjie-multiplatform）對上游

這份文件回答「為什麼 Windows／Linux 版跟上游行為不一樣、大詞庫好像少了東西」，並列出舊版所有與上游不同的行為，作為之後決定要加哪些 overlay 設定的依據。

- 舊版：`shanjie-multiplatform`（本機 `~/projects/shanjie-windows`），Windows 0.10.2、Linux 0.10.2，上游 pin 為 `b2f5da8`（2026-10-16，v0.4.6）。
- 上游：`Nanako0129/shanjie` main `c0d1c64`（2026-10-18），比 `b2f5da8` 新 339 個 commit，最近的 release 是 v0.4.7。
- 新移植版（這個 repo 的 `ports/`）：以 `c0d1c64` 為底，之後合併到上游 `1d6547d`（2026-10-19），`core/` 只加了 Windows 的作業系統 shim（`ports/docs/core-windows.md`），shell 由 Swift 逐行移植，資料夾內容與上游 macOS app 的 Resources 完全相同。

來源標示：**〔查〕** 是這次直接查過程式、資料或設定範本；**〔文〕** 是依舊版 README、UPSTREAM-CHANGES.md 等文件的描述，沒有逐一重跑。

## 一、結論

使用者感覺到的落差主要有三個來源，彼此疊加：

1. **資料少了上游 macOS 版出貨的六份檔案**〔查〕。少了 ACG 詞包（44,911 詞）、數字表、KN 側檔、語意校正模型、書面三元模型，英文詞表也換成了另一份。所以「大詞庫少詞」是真的：動漫與遊戲詞整包不在，語意校正與書面三元模型也都沒有。
2. **預設值不同**〔查〕。舊版新安裝時預設用「Windows 操作」範本（Linux 也套這份），裡面即時預測關閉、排序固定為聊天、上游行內英文關閉；上游則是即時預測開啟、依 App 切換聊天／書面、中英混打開啟。
3. **舊版加了很多自己的輸入行為**〔查〕〔文〕。單按 Shift 切換中英、候選窗在游標右側、Enter 只提交游標前、Windows 注音順序混打、拼音、Big5 過濾、聯想、加詞浮層、標點遮罩等等。上游都沒有，而且大多直接寫進核心（`platform/*.rs` 在建置時覆蓋上游 core）。

另外，舊版的上游 pin 比現在晚了 339 個 commit，中間上游新增的功能舊版都沒有（見第三節）。

一個可以直接驗證的例子是 Selftest.swift 的 `row284`：鍵序 `tj6z;6vup 5j; xk7u ek7xm4gjo3cj6`。

| | 聊天 | 書面 |
| --- | --- | --- |
| 上游 macOS／新移植版 | 廚房新裝了一個率水壺 | 廚房新裝了一個**律**水壺 |
| 舊移植版 | 預設固定聊天（`ranking=chat`），所以每個 App 都是這一句〔查〕 | 改成書面也沒有 trigram（舊文件記為 `tri: None`〔文〕；上游 Selftest 註明律水壺需要 trigram.sjt3） |

新移植版在 Linux 上以真資料、經由私有 ibus-daemon 端對端跑過這兩句（`ports/linux/tests/scenarios/row284-*.json`），結果與上游相同。

## 二、資料與模型

上游 macOS app 的 Resources，以 `scripts/build-app.sh` 為準：

| 檔案 | 內容 | 舊移植版 | 新移植版 |
| --- | --- | --- | --- |
| mcbpmf-data.txt、overlay-add.tsv、sandhi-add.tsv、names-add.tsv、demote.tsv | 主詞庫與覆蓋層 | 有〔查〕 | 有 |
| digits.tsv | 數字字母鍵打出數字（2026年、3.14），107,543 行 | **無**〔查〕；這個功能也在 pin 之後才加入上游 | 有 |
| packs/acg-add.tsv、acg-sources.tsv、acg.json | 動漫與遊戲詞包，44,911 詞，選單「動漫與遊戲詞」，預設開 | **無**〔查〕；`b2f5da8` 時上游就已出貨 | 有 |
| bigram.sjlm（model-v6）＋classes.sjc（classes-v5） | 語言模型 | 改成舊版自己的映射格式（SJWM），由 v6／v5 轉出〔查〕 | 上游原檔 |
| kn.sjkn（kn-v1） | Kneser–Ney 側檔 | **無**：「未接入」〔文〕 | 有 |
| tinylm.sjtl（sj-sr-6-14m） | 語意校正（rerank）模型，選單「語意校正」，預設開 | **無**；`b2f5da8` 時上游出貨的是 tinylm-v5，現在換成 sj-sr-6-14m | 有 |
| trigram.sjt3（sj-cx-1-7.7m） | 書面設定的三元模型（律水壺） | **無**：映射模型 `tri: None`〔文〕 | 有 |
| english-words.txt | 中英混打的英文詞表，由 SymSpell 產生 | **換成** Rime Easy English 抽出的 english-common.txt（LGPL）〔查〕 | 上游原檔，同一支產生程式並核對雜湊 |
| LICENSES 與兩份 attribution | 授權 | 舊版自己的一份 | 從 build-app.sh 擷取，與上游逐字相同 |

舊版另外多帶了 pinyin-map.tsv（拼音）、names-*.tsv 的中間檔，以及線上下載的「使用者模型」〔查〕。

## 三、上游在舊 pin 之後才有的行為（舊版全都沒有）

依 `git log b2f5da8..c0d1c64`〔查〕，與輸入行為有關的有：

- **數字混打**（digits）：英文混打開著時，數字列的鍵由前文判斷是數字還是注音，並配上 digits.tsv。後續還有 Down／Enter／Shift+Enter 依畫面結束數字段（A.12）。
- **語意校正換成 sj-sr-6-14m**，書面強度重新選定（rerank-formal）。
- **書面三元模型**（trigram.sjt3）真正出貨。
- **repick-freeze**：從預測列選字時保留選取範圍左邊的詞；Backspace 拆開的片段在打下一個音節時釋放。
- 其餘多為評測工具與文件。

`c0d1c64..1d6547d`（盤點之後才合併進來）只有一項行為：

- **中英文之間自動加空格**（cjk-spacing，「盤古之白」）：送出時在漢字和半形英文、數字之間加一個空白，開頭也看游標前一個字。預設關，只在設定視窗。移植版照搬（`ports/shell/src/spacing.rs`），放在選單的設定項目裡。

## 四、預設值：舊版範本與上游

舊版新安裝時用 `interaction=windows`（`shared/src/profile.rs` 的預設），Linux 用同一份範本（`linux/common/src/paths.rs` 的 `TEMPLATE = profiles/windows.ini`）〔查〕。另一份 `profiles/shanjie.ini`（「善解操作」）要手動套用。

| 項目 | 上游 | 舊版 Windows 範本（預設） | 舊版善解範本 |
| --- | --- | --- | --- |
| 即時預測 | 開 | **關**（`prediction=0`） | 開 |
| 聊天／書面 | 依 App 自動（聊天 App 用聊天，其他與未知 App 用書面），選單可個別指定 | **固定一種**（`ranking=chat`），沒有依 App 切換 | 固定 `chat` |
| 中英混打（上游英文） | 開 | **關**（`upstream_english=0`） | 關 |
| 英文顯示 | 打字時就顯示 | `english_inline=1`（相同） | 相同 |
| 避免敏感字詞排在前面 | 開 | 開（`demote=1`） | 開 |
| 語意校正 | 開，強度標準 | **沒有這個功能** | 沒有 |
| 動漫與遊戲詞 | 開 | **沒有這個功能** | 沒有 |
| 學習 | 一律學；安全輸入、密碼 App 與無法辨識的 App 暫停 | `learning=1`，沒有依 App 暫停 | **`learning=0`** |
| 候選窗方向 | 橫排，可改直排 | 橫向膠囊，沒有直排／格狀〔文〕 | 相同 |
| 候選範圍 | 游標左側（上游 core） | **游標右側**（`anchor=right`） | 左側 |
| 開啟候選 | ↓ 或空白 | **只有 ↓**（`open=down`） | 兩者 |
| Enter | 提交整句 | **只提交游標前**（`enter_to_cursor=1`）、選字後直接提交（`enter_commits=1`） | 整句 |
| ↑ | 上游行為 | **循環**（`up_cycles=1`） | 上游 |
| 單按 Shift | 上游沒有中英切換（macOS 以切換輸入來源處理） | **切換中英**（`shift_toggle=1`） | 關 |
| Shift+空白 | 上游行為 | **全半形切換**（`shift_space_toggle=1`） | 關 |
| Ctrl+標點 | 上游行為 | **全形標點**（`ctrl_punctuation=1`） | 關 |
| Shift／直接標點 | 上游 core 的標點表 | **全部遮罩**（`[punctuation]` 全 0：不出全形標點） | 全開 |
| 提交後額外空白 | 無（中英之間自動加空格是另一回事，預設關） | 關 | **開**（`space_after_commit=1`） |
| 獨立聲調鍵 | 上游行為 | 關（`standalone_tones=0`） | 開 |
| Ctrl+\ | 上游 core 使用 | **關**（`ctrl_backslash=0`） | 開 |
| 英數模式 | 無 | `alphanumeric=1` | 0 |
| 代碼輸入、符號鍵盤 | 無 | **開**（`code_input=1`、`symbol_keyboard=1`） | 關 |
| 雙 Ctrl 加詞 | 無 | 開（`add_word=1`） | 開 |
| 中／英狀態提示 | 無 | 開（`mode_indicator=1`） | 開 |

## 五、舊版自有的行為（上游沒有的）

依類別列出。「怎麼做的」欄說明舊版把它放在哪裡，這會影響移植到新架構的成本。

### 1. 切換與快捷鍵
- 單按 Shift 切換中英，可選左右側（`shift_toggle`、`shift_side`）〔查〕
- Ctrl+空白、Shift+空白切換（`ctrl_space_toggle`、`shift_space_toggle`）〔查〕
- Caps Lock 打英文（`caps_lock_english`）〔查〕
- 全形／半形（`full_width`）、Ctrl+標點出全形標點（`ctrl_punctuation`）〔查〕
- 反引號叫出符號（`backquote_symbols`）、代碼輸入（`code_input`）、符號鍵盤（`symbol_keyboard`）〔查〕
- Esc 分段清除（`escape=staged`）〔查〕
- 刪除候選（`forget_candidate`；上游另有 ⌘⌫ 忘記反白候選）〔查〕
- 中／英狀態提示（`mode_indicator`），Linux 顯示在頂端列〔文〕

### 2. 組字與混打（多數改在核心：`platform/*.rs`）
- **Windows 注音順序混打**（`mixed_input`）：未完成時先顯示原始按鍵（例如 `gj`）並在下方提示 `ㄕㄨ`，聲調完成才轉中文，Enter 保留原文；數字或符號開頭的序列允許亂序〔文〕。設定頁另有「上游英文詞表／兩者並用」，共四種〔文〕
- 混打容錯：四鍵內一次相鄰重複（`mixed_tolerance`）〔文〕
- 字面數字與標點留在組字區（`composition_windows.rs`）〔文〕
- 空白相關：`space_commits`、`space_after_commit`、`space_inputs_zhuyin`、`pinyin_space_converts`、`space_pages`、`space_operates`〔查〕
- 獨立聲調鍵（`standalone_tones`）〔查〕
- 拼音輸入（`keyboard` 的拼音選項，配 pinyin-map.tsv）〔查〕
- 曾經有過的免聲調／簡拼，已移除〔文〕

### 3. 候選
- 候選範圍在游標右側（`anchor=right`），上游是左側〔查〕
- 開啟方式只有 ↓、只有空白或兩者（`open`）〔查〕
- Enter：選字後直接提交（`enter_commits`）、只提交游標前（`enter_to_cursor`）〔查〕
- ↑ 循環（`up_cycles`）、翻頁循環（`wrap_pages`）、Backspace 刪候選（`backspace_deletes`）〔查〕
- Big5 字集過濾（`big5`）〔查〕
- 選字後聯想（`associations`），舊版自己做的〔查〕
- 沒有直排與展開格狀〔文〕
- 外觀：主題（系統／淺／深、高對比）、字級 14–32〔文〕

### 4. 標點
- 每個 Shift 標點與直接標點都可個別關閉（`[punctuation]` 十九個開關），Windows 範本全部關閉〔查〕

### 5. 詞庫、模型與學習
- 個人詞庫（dictionary-*.tsv，設定頁編輯）〔查〕
- 雙 Ctrl 反白加詞浮層，加入前比對內建詞庫、個人詞庫與模型〔文〕
- 線上模型清單與下載、選用其他模型配套（`model_id`）〔文〕
- 學習整個關閉（`learning`）、學習用前文（`context`）〔查〕
- 映射模型格式（SJWM0002）與多程序共用快取〔文〕

### 6. 平台與程式架構
- Windows：TSF DLL 在 App 程序內直接跑核心，另有常駐預載程序與 AppContainer 用的 broker；x86／x64 兩套；WinUI 3 設定程式；Rust 安裝管理程式與完整回復交易〔文〕
- 兩種鍵盤（標準／倚天）各註冊成一個 Windows 輸入法 profile；上游是同一個輸入法、選單切換〔查〕
- Linux：GTK4／libadwaita 設定程式，設定存 `~/.local/share/shanjie/profile.ini`〔查〕
- 兩個平台共用 `shared/` 的輸入決策；所有核心修改由 `platform/` 與 prepare 腳本在建置時覆蓋上游 core〔文〕

## 六、上游有、舊版沒有或不一樣的行為

除了第二、三節的資料與新功能，上游 shell（Shell.swift、Learning.swift）本身的行為舊版也大多沒有：

- **依 App 切換聊天／書面**：聊天 App 清單、選單「這個 App 用聊天設定」、設定視窗「切換方式」（依 App 自動／一律聊天／一律書面）與「個別設定的 App」〔查〕
- **學習閘門**：安全輸入、密碼管理 App 清單、無法辨識的 App 一律暫停學習，選單顯示暫停原因〔查〕
- **左側前文**：游標前最多兩個漢字（32 個 UTF-16 單位內）送進核心〔查〕
- **組字擁有者**：同一時間只有一個 App 擁有組字；在另一個 App 按鍵，先把組字提交回原本的 App〔查〕
- **語意校正的背景佇列**：一次只跑一個，最新的在後面等，結果只套用到原本的引擎與組字〔查〕
- **英文段提示**：英文段沒有候選時顯示「空白：英文斷開」〔查〕
- **直排候選窗與展開格狀**（翻頁鍵只在直排窗開著時送進核心）〔查〕
- 選單的項目、順序與文字，清除選字記憶的確認與失敗視窗〔查〕

新移植版以上全部照上游做；只有平台做不到的地方不同，列在 `ports/docs/porting.md`。

## 七、給 overlay 設定的分類

新架構的 overlay 有兩層：

- **只改預設值**：`ports/overlay/defaults.ini`（兩個平台共用）、`linux.ini`、`windows.ini`，打包時合併。只影響使用者從沒選過的設定，使用者存過的值永遠優先。目前三份都是空的，等於上游行為。可用的鍵是上游的 13 個 UserDefaults 鍵，例如 `prediction=false`、`profileMode=chat`、`english=false`。
- **新行為**：需要寫程式。原則是放在 shell 或平台層、做成預設關閉的開關，不改 `core/`，這樣上游更新時可以直接合併。

把第四、五節的項目照這個原則分：

| 類別 | 項目 | 做法 |
| --- | --- | --- |
| 改預設值就能做到 | 即時預測關、一律聊天、中英混打關、語意校正關、動漫詞關、直排 | `defaults.ini`，不必寫程式 |
| 平台層開關（不動 core） | 單按 Shift／Ctrl+空白切換中英、Caps Lock 英文、中／英狀態提示、全形空白與標點（提交時轉換）、提交後額外空白、候選窗外觀（字級、主題）、學習整個關閉（shell 的學習閘門多一個條件） | 在 shell 加開關並寫進 settings.ini；兩個平台共用 |
| 需要攔截按鍵再交給 core | Enter 只提交游標前、選字後直接提交、↑ 循環、開啟候選只用 ↓、Esc 分段、標點遮罩、Ctrl+\ 關閉 | 可以在 shell 送進 core 之前改寫或攔下按鍵；做不到的要評估是否得改 core |
| 改 core 才做得到 | 候選範圍在游標右側、Windows 注音順序混打與容錯、字面數字／標點留在組字區、獨立聲調、拼音、Big5 過濾、聯想 | 舊版都是覆蓋 core；建議先確認是否真的需要，需要的話以 patch 形式集中管理 |
| 獨立功能 | 個人詞庫與加詞浮層、線上模型、設定視窗 | 另外規劃 |

## 附：怎麼重現這份盤點

- 舊版設定範本：`~/projects/shanjie-windows/profiles/windows.ini`、`shanjie.ini`
- 舊版資料夾：`~/projects/shanjie-windows/build/linux/data`
- 上游出貨清單：`scripts/build-app.sh`（`ports/data/assemble.sh` 照抄）
- 上游新功能：`git log --oneline b2f5da8..c0d1c64 --no-merges`
