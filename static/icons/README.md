# Skilvella アイコン

Codex組み込みのimagegenで生成した、S字のライブラリカードと検索レンズを組み合わせたアイコンです。背景は透過で、配色はインディゴとティールです。生成時の全文プロンプトは [prompt.txt](prompt.txt) に保存しています。APIキーを使うCLI生成は使用していません。

| ファイル | 用途 |
| --- | --- |
| `skilvella.png` | 生成マスター、1254×1254、透過RGBA |
| `icon-16.png` ～ `icon-1024.png` | 各表示サイズ用の透過PNG |
| `skilvella.ico` | 16・20・24・32・48・64・128・256px入り。favicon・Windowsタスクトレイ向け |
| `skilvella.icns` | Macのアプリアイコン向け |

PNGへのリサイズとICO/ICNSへの形式変換だけを行っています。faviconと画面ヘッダー、Windowsのタスクトレイ、Macのメニューバーはこの素材を使用しています。トレイ用アイコンは実行ファイルに同梱するため、起動時に画像ファイルの配置は不要です。Windows実行ファイル自体のリソースアイコンは未設定です。

PNGとICOの再書き出し: `pwsh -File scripts/Export-Icons.ps1`。MacのICNSは同じPNGから標準の `iconutil` で作成します。
