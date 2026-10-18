善解 Linux（IBus）— 上游 Nanako0129/shanjie 的移植，行為與 macOS 版相同

需求：x86_64，有 IBus 1.5 的發行版（在 Fedora 44 GNOME 上測試）。

安裝：解壓後以一般使用者執行
  ./install.sh
腳本會用 sudo 安裝系統檔，並把「善解」加到 GNOME 輸入來源的最後一項。
會取代先前的移植版（同一個引擎名稱），並移除它的設定程式 shanjie-settings。
安裝到其他位置：PREFIX=/usr/local ./install.sh

設定：輸入法選單（頂端列的「善」）。macOS 設定視窗裡的項目放在選單中「善解設定…」的位置。
選字記憶：~/.local/share/shanjie（與先前的移植版同一份 learning.tsv，會沿用）。
設定值：~/.config/shanjie/settings.ini（先前移植版的 profile.ini 不再讀取）。

移除：./uninstall.sh
個人資料預設保留；./uninstall.sh --delete-user-data 會把它移到 <資料夾>.deleted_files/<時間>，不會直接刪除。

與 macOS 版因平台而不同的地方：ports/docs/porting.md。
