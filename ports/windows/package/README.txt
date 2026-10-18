善解 Windows（TSF）— 上游 Nanako0129/shanjie 的移植，行為與 macOS 版相同

【未在 Windows 實機驗證】這個版本只在 Linux 上交叉編譯（x86_64-pc-windows-gnu）。
host 的邏輯（按鍵、選單、候選窗內容、跨 App 的組字）在 Linux 上以真詞庫測過；
TSF DLL、命名管道、候選窗的繪製與安裝腳本都還沒在 Windows 上跑過。

需求：Windows 10 1809 以上或 Windows 11，x64。

安裝：以系統管理員身分開啟 PowerShell，於此資料夾執行
  powershell -ExecutionPolicy Bypass -File .\install.ps1
之後按 Win+Space 選「善解」。

設定：工作列輸入法圖示（或語言列）上的「善」選單。macOS 設定視窗的項目放在選單中「善解設定…」的位置。
選字記憶與設定值：%LOCALAPPDATA%\Shanjie（與先前的 Windows 移植版同一份 learning.tsv，會沿用；
先前的 profile.ini 不再讀取）。

移除：以系統管理員身分執行
  powershell -ExecutionPolicy Bypass -File "%ProgramFiles%\ShanjiePorts\uninstall.ps1"
加上 -DeleteUserData 會把個人資料移到 %LOCALAPPDATA%\Shanjie.deleted_files\<時間>。

與先前的移植版（shanjie-multiplatform）使用不同的識別碼，可以並存；建議先移除舊版。
與 macOS 版因平台而不同的地方：ports/docs/porting.md。
