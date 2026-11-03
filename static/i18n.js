"use strict";
(() => {
const catalog = {
  "設定": [
    "Settings",
    "设置",
    "設定",
    "설정",
    "Configuración",
    "Paramètres",
    "Einstellungen",
    "Configurações"
  ],
  "表示": [
    "Appearance",
    "显示",
    "顯示",
    "표시",
    "Apariencia",
    "Apparence",
    "Darstellung",
    "Aparência"
  ],
  "言語": [
    "Language",
    "语言",
    "語言",
    "언어",
    "Idioma",
    "Langue",
    "Sprache",
    "Idioma"
  ],
  "自動（ブラウザーに合わせる）": [
    "Automatic (browser language)",
    "自动（浏览器语言）",
    "自動（瀏覽器語言）",
    "자동(브라우저 언어)",
    "Automático (idioma del navegador)",
    "Automatique (langue du navigateur)",
    "Automatisch (Browsersprache)",
    "Automático (idioma do navegador)"
  ],
  "テーマ": [
    "Theme",
    "主题",
    "主題",
    "테마",
    "Tema",
    "Thème",
    "Design",
    "Tema"
  ],
  "自動": [
    "Automatic",
    "自动",
    "自動",
    "자동",
    "Automático",
    "Automatique",
    "Automatisch",
    "Automático"
  ],
  "ライト": [
    "Light",
    "浅色",
    "淺色",
    "밝게",
    "Claro",
    "Clair",
    "Hell",
    "Claro"
  ],
  "ダーク": [
    "Dark",
    "深色",
    "深色",
    "어둡게",
    "Oscuro",
    "Sombre",
    "Dunkel",
    "Escuro"
  ],
  "スキル管理": [
    "Skill management",
    "技能管理",
    "技能管理",
    "스킬 관리",
    "Gestión de habilidades",
    "Gestion des compétences",
    "Skillverwaltung",
    "Gerenciamento de habilidades"
  ],
  "本文へ移動": [
    "Skip to content",
    "跳到正文",
    "跳至內容",
    "본문으로 이동",
    "Ir al contenido",
    "Aller au contenu",
    "Zum Inhalt",
    "Ir para o conteúdo"
  ],
  "メイン": [
    "Main",
    "主导航",
    "主導覽",
    "주 메뉴",
    "Principal",
    "Principal",
    "Hauptnavigation",
    "Principal"
  ],
  "ライブラリ": [
    "Library",
    "库",
    "資料庫",
    "라이브러리",
    "Biblioteca",
    "Bibliothèque",
    "Bibliothek",
    "Biblioteca"
  ],
  "フォルダー": [
    "Folders",
    "文件夹",
    "資料夾",
    "폴더",
    "Carpetas",
    "Dossiers",
    "Ordner",
    "Pastas"
  ],
  "＋ 新規スキル": [
    "＋ New skill",
    "＋ 新建技能",
    "＋ 新增技能",
    "＋ 새 스킬",
    "＋ Nueva habilidad",
    "＋ Nouvelle compétence",
    "＋ Neuer Skill",
    "＋ Nova habilidade"
  ],
  "新規スキル": [
    "New skill",
    "新建技能",
    "新增技能",
    "새 스킬",
    "Nueva habilidad",
    "Nouvelle compétence",
    "Neuer Skill",
    "Nova habilidade"
  ],
  "Skilvella · ローカルライブラリ": [
    "Skilvella · Local library",
    "Skilvella · 本地库",
    "Skilvella · 本機資料庫",
    "Skilvella · 로컬 라이브러리",
    "Skilvella · Biblioteca local",
    "Skilvella · Bibliothèque locale",
    "Skilvella · Lokale Bibliothek",
    "Skilvella · Biblioteca local"
  ],
  "再スキャン": [
    "Rescan",
    "重新扫描",
    "重新掃描",
    "다시 스캔",
    "Volver a escanear",
    "Analyser à nouveau",
    "Erneut scannen",
    "Verificar novamente"
  ],
  "スキャン中…": [
    "Scanning…",
    "正在扫描…",
    "正在掃描…",
    "스캔 중…",
    "Escaneando…",
    "Analyse…",
    "Scan läuft…",
    "Verificando…"
  ],
  "スキルを検索": [
    "Search skills",
    "搜索技能",
    "搜尋技能",
    "스킬 검색",
    "Buscar habilidades",
    "Rechercher des compétences",
    "Skills suchen",
    "Buscar habilidades"
  ],
  "名前・説明・本文から検索": [
    "Search name, description and content",
    "搜索名称、描述和正文",
    "搜尋名稱、說明及內容",
    "이름, 설명 및 본문 검색",
    "Buscar por nombre, descripción y contenido",
    "Rechercher dans le nom, la description et le contenu",
    "Name, Beschreibung und Inhalt durchsuchen",
    "Buscar por nome, descrição e conteúdo"
  ],
  "検索": [
    "Search",
    "搜索",
    "搜尋",
    "검색",
    "Buscar",
    "Rechercher",
    "Suchen",
    "Buscar"
  ],
  "ソース": [
    "Source",
    "来源",
    "來源",
    "소스",
    "Origen",
    "Source",
    "Quelle",
    "Origem"
  ],
  "すべて": [
    "All",
    "全部",
    "全部",
    "전체",
    "Todos",
    "Tous",
    "Alle",
    "Todos"
  ],
  "タグ": [
    "Tags",
    "标签",
    "標籤",
    "태그",
    "Etiquetas",
    "Étiquettes",
    "Tags",
    "Etiquetas"
  ],
  "タグ名": [
    "Tag name",
    "标签名",
    "標籤名稱",
    "태그 이름",
    "Nombre de etiqueta",
    "Nom de l’étiquette",
    "Tagname",
    "Nome da etiqueta"
  ],
  "カテゴリ": [
    "Category",
    "分类",
    "分類",
    "카테고리",
    "Categoría",
    "Catégorie",
    "Kategorie",
    "Categoria"
  ],
  "カテゴリ名": [
    "Category name",
    "分类名称",
    "分類名稱",
    "카테고리 이름",
    "Nombre de categoría",
    "Nom de la catégorie",
    "Kategoriename",
    "Nome da categoria"
  ],
  "状態": [
    "Status",
    "状态",
    "狀態",
    "상태",
    "Estado",
    "État",
    "Status",
    "Status"
  ],
  "公開": [
    "Published",
    "已公开",
    "已公開",
    "공개",
    "Publicada",
    "Publiée",
    "Veröffentlicht",
    "Publicada"
  ],
  "非公開": [
    "Unpublished",
    "未公开",
    "未公開",
    "비공개",
    "No publicada",
    "Non publiée",
    "Nicht veröffentlicht",
    "Não publicada"
  ],
  "エラー": [
    "Error",
    "错误",
    "錯誤",
    "오류",
    "Error",
    "Erreur",
    "Fehler",
    "Erro"
  ],
  "重複": [
    "Duplicate",
    "重复",
    "重複",
    "중복",
    "Duplicada",
    "Doublon",
    "Duplikat",
    "Duplicada"
  ],
  "見つからない": [
    "Missing",
    "未找到",
    "找不到",
    "찾을 수 없음",
    "No encontrada",
    "Introuvable",
    "Nicht gefunden",
    "Não encontrada"
  ],
  "条件を解除": [
    "Clear filters",
    "清除筛选",
    "清除篩選",
    "필터 초기화",
    "Borrar filtros",
    "Effacer les filtres",
    "Filter zurücksetzen",
    "Limpar filtros"
  ],
  "ソースフォルダーを登録してください": [
    "Add a source folder",
    "请添加来源文件夹",
    "請新增來源資料夾",
    "소스 폴더를 추가하세요",
    "Añade una carpeta de origen",
    "Ajoutez un dossier source",
    "Quellordner hinzufügen",
    "Adicione uma pasta de origem"
  ],
  "該当するスキルはありません": [
    "No matching skills",
    "没有匹配的技能",
    "沒有符合的技能",
    "일치하는 스킬이 없습니다",
    "No hay habilidades coincidentes",
    "Aucune compétence correspondante",
    "Keine passenden Skills",
    "Nenhuma habilidade correspondente"
  ],
  "ソースを登録": [
    "Add source",
    "添加来源",
    "新增來源",
    "소스 추가",
    "Añadir origen",
    "Ajouter une source",
    "Quelle hinzufügen",
    "Adicionar origem"
  ],
  "公開状態": [
    "Publication",
    "公开状态",
    "公開狀態",
    "공개 상태",
    "Publicación",
    "Publication",
    "Veröffentlichung",
    "Publicação"
  ],
  "変更しない": [
    "No change",
    "不更改",
    "不變更",
    "변경 안 함",
    "Sin cambios",
    "Aucun changement",
    "Nicht ändern",
    "Sem alteração"
  ],
  "公開にする": [
    "Publish",
    "公开",
    "公開",
    "공개로 설정",
    "Publicar",
    "Publier",
    "Veröffentlichen",
    "Publicar"
  ],
  "非公開にする": [
    "Unpublish",
    "取消公开",
    "取消公開",
    "비공개로 설정",
    "Dejar de publicar",
    "Dépublier",
    "Veröffentlichung aufheben",
    "Despublicar"
  ],
  "タグを追加": [
    "Add tags",
    "添加标签",
    "新增標籤",
    "태그 추가",
    "Añadir etiquetas",
    "Ajouter des étiquettes",
    "Tags hinzufügen",
    "Adicionar etiquetas"
  ],
  "カンマ区切り": [
    "Comma separated",
    "用逗号分隔",
    "以逗號分隔",
    "쉼표로 구분",
    "Separadas por comas",
    "Séparées par des virgules",
    "Durch Kommas getrennt",
    "Separadas por vírgulas"
  ],
  "選択項目に適用": [
    "Apply to selected",
    "应用到所选项",
    "套用至所選項目",
    "선택 항목에 적용",
    "Aplicar a la selección",
    "Appliquer à la sélection",
    "Auf Auswahl anwenden",
    "Aplicar à seleção"
  ],
  "更新中…": [
    "Updating…",
    "正在更新…",
    "正在更新…",
    "업데이트 중…",
    "Actualizando…",
    "Mise à jour…",
    "Wird aktualisiert…",
    "Atualizando…"
  ],
  "スキル": [
    "Skills",
    "技能",
    "技能",
    "스킬",
    "Habilidades",
    "Compétences",
    "Skills",
    "Habilidades"
  ],
  "ソース / カテゴリ": [
    "Source / Category",
    "来源 / 分类",
    "來源 / 分類",
    "소스 / 카테고리",
    "Origen / Categoría",
    "Source / Catégorie",
    "Quelle / Kategorie",
    "Origem / Categoria"
  ],
  "最終スキャン:": [
    "Last scan:",
    "上次扫描：",
    "上次掃描：",
    "마지막 스캔:",
    "Último escaneo:",
    "Dernière analyse :",
    "Letzter Scan:",
    "Última verificação:"
  ],
  "未実行": [
    "Never",
    "尚未执行",
    "尚未執行",
    "실행 전",
    "Nunca",
    "Jamais",
    "Noch nie",
    "Nunca"
  ],
  "前の50件": [
    "Previous 50",
    "前50项",
    "前50項",
    "이전 50개",
    "50 anteriores",
    "50 précédentes",
    "Vorherige 50",
    "50 anteriores"
  ],
  "次の50件": [
    "Next 50",
    "后50项",
    "後50項",
    "다음 50개",
    "50 siguientes",
    "50 suivantes",
    "Nächste 50",
    "Próximas 50"
  ],
  "表示中のスキルをすべて選択": [
    "Select all displayed skills",
    "选择所有显示的技能",
    "選取所有顯示的技能",
    "표시된 모든 스킬 선택",
    "Seleccionar todas las habilidades visibles",
    "Sélectionner toutes les compétences affichées",
    "Alle angezeigten Skills auswählen",
    "Selecionar todas as habilidades exibidas"
  ],
  "← ライブラリ": [
    "← Library",
    "← 库",
    "← 資料庫",
    "← 라이브러리",
    "← Biblioteca",
    "← Bibliothèque",
    "← Bibliothek",
    "← Biblioteca"
  ],
  "ライブラリへ": [
    "Go to library",
    "返回库",
    "返回資料庫",
    "라이브러리로",
    "Ir a la biblioteca",
    "Aller à la bibliothèque",
    "Zur Bibliothek",
    "Ir para a biblioteca"
  ],
  "公開設定あり・再スキャン待ち": [
    "Published setting · awaiting rescan",
    "已设为公开 · 等待重新扫描",
    "已設為公開 · 等待重新掃描",
    "공개 설정됨 · 다시 스캔 대기",
    "Publicada · pendiente de escaneo",
    "Publication activée · analyse requise",
    "Veröffentlicht · Scan ausstehend",
    "Publicada · aguardando verificação"
  ],
  "公開設定あり・現在は取得対象外": [
    "Published setting · currently unavailable",
    "已设为公开 · 当前无法获取",
    "已設為公開 · 目前無法取得",
    "공개 설정됨 · 현재 조회 불가",
    "Publicada · no disponible actualmente",
    "Publication activée · indisponible actuellement",
    "Veröffentlicht · derzeit nicht abrufbar",
    "Publicada · indisponível no momento"
  ],
  "ライブラリで再スキャン": [
    "rescan in the library",
    "在库中重新扫描",
    "在資料庫中重新掃描",
    "라이브러리에서 다시 스캔",
    "volver a escanear en la biblioteca",
    "relancer l’analyse dans la bibliothèque",
    "in der Bibliothek erneut scannen",
    "verificar novamente na biblioteca"
  ],
  "してください。": [
    ".",
    "。",
    "。",
    "하세요.",
    ".",
    ".",
    ".",
    "."
  ],
  "最新版を開く": [
    "Open latest version",
    "打开最新版本",
    "開啟最新版本",
    "최신 버전 열기",
    "Abrir la última versión",
    "Ouvrir la dernière version",
    "Aktuelle Version öffnen",
    "Abrir a versão mais recente"
  ],
  "（先に下書きをコピーしてください）": [
    "(copy your draft first)",
    "（请先复制草稿）",
    "（請先複製草稿）",
    "(먼저 초안을 복사하세요)",
    "(copia primero el borrador)",
    "(copiez d’abord le brouillon)",
    "(Entwurf vorher kopieren)",
    "(copie o rascunho primeiro)"
  ],
  "読み取りエラー:": [
    "Read error:",
    "读取错误：",
    "讀取錯誤：",
    "읽기 오류:",
    "Error de lectura:",
    "Erreur de lecture :",
    "Lesefehler:",
    "Erro de leitura:"
  ],
  "読み取りエラー": [
    "Read errors",
    "读取错误",
    "讀取錯誤",
    "읽기 오류",
    "Errores de lectura",
    "Erreurs de lecture",
    "Lesefehler",
    "Erros de leitura"
  ],
  "未保存の下書き": [
    "Unsaved draft",
    "未保存的草稿",
    "未儲存的草稿",
    "저장되지 않은 초안",
    "Borrador sin guardar",
    "Brouillon non enregistré",
    "Ungespeicherter Entwurf",
    "Rascunho não salvo"
  ],
  "最後のスキャン内容": [
    "Last scanned content",
    "上次扫描的内容",
    "上次掃描的內容",
    "마지막 스캔 내용",
    "Contenido del último escaneo",
    "Contenu de la dernière analyse",
    "Zuletzt gescannter Inhalt",
    "Conteúdo da última verificação"
  ],
  "保存済み": [
    "Saved",
    "已保存",
    "已儲存",
    "저장됨",
    "Guardado",
    "Enregistré",
    "Gespeichert",
    "Salvo"
  ],
  "SKILL.md の全文": [
    "Full SKILL.md content",
    "SKILL.md 全文",
    "SKILL.md 全文",
    "SKILL.md 전체 내용",
    "Contenido completo de SKILL.md",
    "Contenu intégral de SKILL.md",
    "Vollständiger SKILL.md-Inhalt",
    "Conteúdo completo de SKILL.md"
  ],
  "SKILL.md の全文（未保存）": [
    "Full SKILL.md content (unsaved)",
    "SKILL.md 全文（未保存）",
    "SKILL.md 全文（未儲存）",
    "SKILL.md 전체 내용(저장되지 않음)",
    "Contenido de SKILL.md (sin guardar)",
    "Contenu de SKILL.md (non enregistré)",
    "SKILL.md-Inhalt (ungespeichert)",
    "Conteúdo de SKILL.md (não salvo)"
  ],
  "保存中…": [
    "Saving…",
    "正在保存…",
    "正在儲存…",
    "저장 중…",
    "Guardando…",
    "Enregistrement…",
    "Wird gespeichert…",
    "Salvando…"
  ],
  "本文を再作成": [
    "Recreate content",
    "重新创建正文",
    "重新建立內容",
    "본문 다시 만들기",
    "Recrear contenido",
    "Recréer le contenu",
    "Inhalt neu erstellen",
    "Recriar conteúdo"
  ],
  "本文を保存": [
    "Save content",
    "保存正文",
    "儲存內容",
    "본문 저장",
    "Guardar contenido",
    "Enregistrer le contenu",
    "Inhalt speichern",
    "Salvar conteúdo"
  ],
  "プレビューを更新": [
    "Refresh preview",
    "刷新预览",
    "更新預覽",
    "미리 보기 새로 고침",
    "Actualizar vista previa",
    "Actualiser l’aperçu",
    "Vorschau aktualisieren",
    "Atualizar prévia"
  ],
  "Markdown プレビュー": [
    "Markdown preview",
    "Markdown 预览",
    "Markdown 預覽",
    "Markdown 미리 보기",
    "Vista previa Markdown",
    "Aperçu Markdown",
    "Markdown-Vorschau",
    "Prévia Markdown"
  ],
  "管理情報": [
    "Metadata",
    "管理信息",
    "管理資訊",
    "관리 정보",
    "Metadatos",
    "Métadonnées",
    "Metadaten",
    "Metadados"
  ],
  "MCP に公開する": [
    "Publish to MCP",
    "公开到 MCP",
    "公開至 MCP",
    "MCP에 공개",
    "Publicar en MCP",
    "Publier via MCP",
    "Über MCP veröffentlichen",
    "Publicar no MCP"
  ],
  "管理情報を保存": [
    "Save metadata",
    "保存管理信息",
    "儲存管理資訊",
    "관리 정보 저장",
    "Guardar metadatos",
    "Enregistrer les métadonnées",
    "Metadaten speichern",
    "Salvar metadados"
  ],
  "関連ファイル": [
    "Related files",
    "相关文件",
    "相關檔案",
    "관련 파일",
    "Archivos relacionados",
    "Fichiers associés",
    "Zugehörige Dateien",
    "Arquivos relacionados"
  ],
  "変更履歴": [
    "History",
    "修改历史",
    "變更紀錄",
    "변경 이력",
    "Historial",
    "Historique",
    "Verlauf",
    "Histórico"
  ],
  "履歴はまだありません。": [
    "No history yet.",
    "暂无历史记录。",
    "尚無歷史紀錄。",
    "아직 이력이 없습니다.",
    "Todavía no hay historial.",
    "Aucun historique pour le moment.",
    "Noch kein Verlauf vorhanden.",
    "Ainda não há histórico."
  ],
  "復元中…": [
    "Restoring…",
    "正在恢复…",
    "正在還原…",
    "복원 중…",
    "Restaurando…",
    "Restauration…",
    "Wird wiederhergestellt…",
    "Restaurando…"
  ],
  "この版に復元": [
    "Restore this version",
    "恢复此版本",
    "還原此版本",
    "이 버전으로 복원",
    "Restaurar esta versión",
    "Restaurer cette version",
    "Diese Version wiederherstellen",
    "Restaurar esta versão"
  ],
  "名前": [
    "Name",
    "名称",
    "名稱",
    "이름",
    "Nombre",
    "Nom",
    "Name",
    "Nome"
  ],
  "作成先ソース": [
    "Destination source",
    "目标来源",
    "目標來源",
    "대상 소스",
    "Origen de destino",
    "Source de destination",
    "Zielquelle",
    "Origem de destino"
  ],
  "新しいフォルダー名": [
    "New folder name",
    "新文件夹名称",
    "新資料夾名稱",
    "새 폴더 이름",
    "Nombre de la nueva carpeta",
    "Nom du nouveau dossier",
    "Neuer Ordnername",
    "Nome da nova pasta"
  ],
  "作成中…": [
    "Creating…",
    "正在创建…",
    "正在建立…",
    "만드는 중…",
    "Creando…",
    "Création…",
    "Wird erstellt…",
    "Criando…"
  ],
  "スキルを作成": [
    "Create skill",
    "创建技能",
    "建立技能",
    "스킬 만들기",
    "Crear habilidad",
    "Créer une compétence",
    "Skill erstellen",
    "Criar habilidade"
  ],
  "作成先のソースが必要です": [
    "A destination source is required",
    "需要目标来源",
    "需要目標來源",
    "대상 소스가 필요합니다",
    "Se necesita un origen de destino",
    "Une source de destination est nécessaire",
    "Eine Zielquelle ist erforderlich",
    "É necessária uma origem de destino"
  ],
  "スキルフォルダー": [
    "Skill folders",
    "技能文件夹",
    "技能資料夾",
    "스킬 폴더",
    "Carpetas de habilidades",
    "Dossiers de compétences",
    "Skillordner",
    "Pastas de habilidades"
  ],
  "MCP接続先": [
    "MCP endpoint",
    "MCP 端点",
    "MCP 端點",
    "MCP 연결 주소",
    "Dirección MCP",
    "Adresse MCP",
    "MCP-Endpunkt",
    "Endereço MCP"
  ],
  "/mcp（この画面と同じ接続先）": [
    "/mcp (same host as this page)",
    "/mcp（与此页面相同的主机）",
    "/mcp（與此頁面相同的主機）",
    "/mcp(이 화면과 같은 호스트)",
    "/mcp (mismo servidor que esta página)",
    "/mcp (même hôte que cette page)",
    "/mcp (gleicher Host wie diese Seite)",
    "/mcp (mesmo servidor desta página)"
  ],
  "移動前の確認事項": [
    "Before moving",
    "移动前须知",
    "移動前須知",
    "이동 전 확인 사항",
    "Antes de mover",
    "Avant le déplacement",
    "Vor dem Verschieben",
    "Antes de mover"
  ],
  "管理中のフォルダー": [
    "Managed folders",
    "管理的文件夹",
    "管理中的資料夾",
    "관리 중인 폴더",
    "Carpetas gestionadas",
    "Dossiers gérés",
    "Verwaltete Ordner",
    "Pastas gerenciadas"
  ],
  "既定": [
    "Default",
    "默认",
    "預設",
    "기본",
    "Predeterminada",
    "Par défaut",
    "Standard",
    "Padrão"
  ],
  "現在の場所": [
    "Current location",
    "当前位置",
    "目前位置",
    "현재 위치",
    "Ubicación actual",
    "Emplacement actuel",
    "Aktueller Speicherort",
    "Local atual"
  ],
  "件": [
    " items",
    " 项",
    " 項",
    "개",
    " elementos",
    " éléments",
    " Einträge",
    " itens"
  ],
  "MCP専用の保存先へ移動": [
    "Move to an MCP-only location",
    "移动到 MCP 专用位置",
    "移至 MCP 專用位置",
    "MCP 전용 위치로 이동",
    "Mover a una ubicación exclusiva de MCP",
    "Déplacer vers un emplacement réservé à MCP",
    "An einen reinen MCP-Speicherort verschieben",
    "Mover para um local exclusivo do MCP"
  ],
  "保存場所を移動": [
    "Move storage location",
    "移动存储位置",
    "移動儲存位置",
    "저장 위치 이동",
    "Mover ubicación",
    "Déplacer l’emplacement",
    "Speicherort verschieben",
    "Mover local de armazenamento"
  ],
  "参照するフォルダーを変更": [
    "Change referenced folder",
    "更改引用的文件夹",
    "變更參照資料夾",
    "참조 폴더 변경",
    "Cambiar carpeta de referencia",
    "Changer le dossier référencé",
    "Referenzierten Ordner ändern",
    "Alterar pasta de referência"
  ],
  "管理から外す": [
    "Stop managing",
    "停止管理",
    "停止管理",
    "관리에서 제외",
    "Dejar de gestionar",
    "Ne plus gérer",
    "Verwaltung beenden",
    "Parar de gerenciar"
  ],
  "解除中…": [
    "Removing…",
    "正在解除…",
    "正在解除…",
    "해제 중…",
    "Quitando…",
    "Suppression…",
    "Wird entfernt…",
    "Removendo…"
  ],
  "ファイルは削除しません": [
    "Files will not be deleted",
    "不会删除文件",
    "不會刪除檔案",
    "파일은 삭제되지 않습니다",
    "Los archivos no se eliminarán",
    "Les fichiers ne seront pas supprimés",
    "Dateien werden nicht gelöscht",
    "Os arquivos não serão excluídos"
  ],
  "フォルダーを追加": [
    "Add folder",
    "添加文件夹",
    "新增資料夾",
    "폴더 추가",
    "Añadir carpeta",
    "Ajouter un dossier",
    "Ordner hinzufügen",
    "Adicionar pasta"
  ],
  "フォルダーの場所（絶対パス）": [
    "Folder location (absolute path)",
    "文件夹位置（绝对路径）",
    "資料夾位置（絕對路徑）",
    "폴더 위치(절대 경로)",
    "Ubicación de carpeta (ruta absoluta)",
    "Emplacement du dossier (chemin absolu)",
    "Ordnerpfad (absolut)",
    "Local da pasta (caminho absoluto)"
  ],
  "追加してスキャン": [
    "Add and scan",
    "添加并扫描",
    "新增並掃描",
    "추가 및 스캔",
    "Añadir y escanear",
    "Ajouter et analyser",
    "Hinzufügen und scannen",
    "Adicionar e verificar"
  ],
  "追加中…": [
    "Adding…",
    "正在添加…",
    "正在新增…",
    "추가 중…",
    "Añadiendo…",
    "Ajout…",
    "Wird hinzugefügt…",
    "Adicionando…"
  ],
  "既定のフォルダーを指定する": [
    "Set default folder",
    "指定默认文件夹",
    "指定預設資料夾",
    "기본 폴더 지정",
    "Establecer carpeta predeterminada",
    "Définir le dossier par défaut",
    "Standardordner festlegen",
    "Definir pasta padrão"
  ],
  "参照するフォルダー（絶対パス）": [
    "Referenced folder (absolute path)",
    "引用的文件夹（绝对路径）",
    "參照資料夾（絕對路徑）",
    "참조 폴더(절대 경로)",
    "Carpeta de referencia (ruta absoluta)",
    "Dossier référencé (chemin absolu)",
    "Referenzordner (absoluter Pfad)",
    "Pasta de referência (caminho absoluto)"
  ],
  "参照先を変更": [
    "Change reference",
    "更改引用位置",
    "變更參照位置",
    "참조 위치 변경",
    "Cambiar referencia",
    "Changer la référence",
    "Referenz ändern",
    "Alterar referência"
  ],
  "変更中…": [
    "Changing…",
    "正在更改…",
    "正在變更…",
    "변경 중…",
    "Cambiando…",
    "Modification…",
    "Wird geändert…",
    "Alterando…"
  ],
  "移動先（絶対パス）": [
    "Destination (absolute path)",
    "目标位置（绝对路径）",
    "目標位置（絕對路徑）",
    "대상 위치(절대 경로)",
    "Destino (ruta absoluta)",
    "Destination (chemin absolu)",
    "Ziel (absoluter Pfad)",
    "Destino (caminho absoluto)"
  ],
  "移動中…": [
    "Moving…",
    "正在移动…",
    "正在移動…",
    "이동 중…",
    "Moviendo…",
    "Déplacement…",
    "Wird verschoben…",
    "Movendo…"
  ],
  "この保存先へ移動": [
    "Move to this location",
    "移动到此位置",
    "移至此位置",
    "이 위치로 이동",
    "Mover a esta ubicación",
    "Déplacer ici",
    "Hierhin verschieben",
    "Mover para este local"
  ],
  "操作を完了できませんでした": [
    "The operation could not be completed",
    "无法完成操作",
    "無法完成操作",
    "작업을 완료하지 못했습니다",
    "No se pudo completar la operación",
    "L’opération n’a pas pu être terminée",
    "Der Vorgang konnte nicht abgeschlossen werden",
    "Não foi possível concluir a operação"
  ],
  "接続・権限の復旧後に保存を再試行": [
    "Retry saving after restoring access",
    "恢复连接和权限后重试保存",
    "恢復連線和權限後重試儲存",
    "연결 및 권한 복구 후 저장 재시도",
    "Reintentar guardar tras restaurar el acceso",
    "Réessayer après rétablissement de l’accès",
    "Nach Wiederherstellung des Zugriffs erneut speichern",
    "Tentar salvar após restaurar o acesso"
  ],
  "未保存の変更": [
    "Unsaved changes",
    "未保存的修改",
    "未儲存的變更",
    "저장되지 않은 변경 사항",
    "Cambios sin guardar",
    "Modifications non enregistrées",
    "Ungespeicherte Änderungen",
    "Alterações não salvas"
  ],
  "操作の確認": [
    "Confirm operation",
    "确认操作",
    "確認操作",
    "작업 확인",
    "Confirmar operación",
    "Confirmer l’opération",
    "Vorgang bestätigen",
    "Confirmar operação"
  ],
  "キャンセル": [
    "Cancel",
    "取消",
    "取消",
    "취소",
    "Cancelar",
    "Annuler",
    "Abbrechen",
    "Cancelar"
  ],
  "続行する": [
    "Continue",
    "继续",
    "繼續",
    "계속",
    "Continuar",
    "Continuer",
    "Fortfahren",
    "Continuar"
  ],
  "{count}件選択": [
    "{count} selected",
    "已选择 {count} 项",
    "已選取 {count} 項",
    "{count}개 선택",
    "{count} seleccionadas",
    "{count} sélectionnées",
    "{count} ausgewählt",
    "{count} selecionadas"
  ],
  "例: my-new-skill": [
    "e.g. my-new-skill",
    "例如：my-new-skill",
    "例如：my-new-skill",
    "예: my-new-skill",
    "p. ej. my-new-skill",
    "p. ex. my-new-skill",
    "z. B. my-new-skill",
    "ex.: my-new-skill"
  ],
  "例: personal": [
    "e.g. personal",
    "例如：personal",
    "例如：personal",
    "예: personal",
    "p. ej. personal",
    "p. ex. personal",
    "z. B. personal",
    "ex.: personal"
  ],
  "例: D:\\Skills または /Users/you/Skills": [
    "e.g. D:\\Skills or /Users/you/Skills",
    "例如：D:\\Skills 或 /Users/you/Skills",
    "例如：D:\\Skills 或 /Users/you/Skills",
    "예: D:\\Skills 또는 /Users/you/Skills",
    "p. ej. D:\\Skills o /Users/you/Skills",
    "p. ex. D:\\Skills ou /Users/you/Skills",
    "z. B. D:\\Skills oder /Users/you/Skills",
    "ex.: D:\\Skills ou /Users/you/Skills"
  ],
  "SKILL.md を含むフォルダーを登録すると、ここで検索・編集できます。": [
    "Add a folder containing SKILL.md files to search and edit them here.",
    "添加包含 SKILL.md 的文件夹，即可在此搜索和编辑。",
    "新增包含 SKILL.md 的資料夾，即可在此搜尋和編輯。",
    "SKILL.md가 포함된 폴더를 추가하면 여기서 검색하고 편집할 수 있습니다.",
    "Añade una carpeta con archivos SKILL.md para buscarlos y editarlos aquí.",
    "Ajoutez un dossier contenant des fichiers SKILL.md pour les rechercher et les modifier ici.",
    "Fügen Sie einen Ordner mit SKILL.md-Dateien hinzu, um diese hier zu suchen und zu bearbeiten.",
    "Adicione uma pasta com arquivos SKILL.md para buscá-los e editá-los aqui."
  ],
  "検索条件を変更するか、ファイルを追加して再スキャンしてください。": [
    "Change the filters or add files and rescan.",
    "请更改筛选条件，或添加文件后重新扫描。",
    "請變更篩選條件，或新增檔案後重新掃描。",
    "검색 조건을 변경하거나 파일을 추가한 후 다시 스캔하세요.",
    "Cambia los filtros o añade archivos y vuelve a escanear.",
    "Modifiez les filtres ou ajoutez des fichiers puis relancez l’analyse.",
    "Ändern Sie die Filter oder fügen Sie Dateien hinzu und scannen Sie erneut.",
    "Altere os filtros ou adicione arquivos e verifique novamente."
  ],
  "公開にすると SKILL.md と、そのフォルダー以下の関連ファイルが MCP の取得対象になります。": [
    "Publishing makes SKILL.md and related files beneath its folder available through MCP.",
    "公开后，MCP 可以获取 SKILL.md 及其文件夹下的相关文件。",
    "公開後，MCP 可以取得 SKILL.md 及其資料夾下的相關檔案。",
    "공개하면 MCP에서 SKILL.md와 해당 폴더 아래의 관련 파일을 조회할 수 있습니다.",
    "Al publicar, SKILL.md y los archivos relacionados de su carpeta estarán disponibles mediante MCP.",
    "La publication rend SKILL.md et les fichiers associés de son dossier accessibles via MCP.",
    "Durch Veröffentlichen werden SKILL.md und zugehörige Dateien unter dessen Ordner über MCP abrufbar.",
    "A publicação disponibiliza SKILL.md e os arquivos relacionados da pasta pelo MCP."
  ],
  "一括操作にはJavaScriptが必要です。個別の管理情報は各スキルから編集できます。": [
    "Bulk operations require JavaScript. You can edit metadata on each skill page.",
    "批量操作需要 JavaScript。可在各技能页面编辑管理信息。",
    "批次操作需要 JavaScript。可在各技能頁面編輯管理資訊。",
    "일괄 작업에는 JavaScript가 필요합니다. 개별 스킬 화면에서 관리 정보를 편집할 수 있습니다.",
    "Las operaciones en lote requieren JavaScript. Puedes editar los metadatos en cada habilidad.",
    "Les opérations groupées nécessitent JavaScript. Modifiez les métadonnées sur la page de chaque compétence.",
    "Stapelvorgänge benötigen JavaScript. Metadaten können auf jeder Skillseite bearbeitet werden.",
    "Operações em lote exigem JavaScript. Edite os metadados na página de cada habilidade."
  ],
  "MCP公開だけではCodexの標準探索から外れません。二重読み込みを避けるには、": [
    "Publishing via MCP does not remove skills from Codex’s default discovery. To avoid loading them twice, ",
    "通过 MCP 公开不会将技能移出 Codex 的默认发现范围。为避免重复加载，请",
    "透過 MCP 公開不會將技能移出 Codex 的預設探索範圍。為避免重複載入，請",
    "MCP에 공개해도 Codex의 기본 탐색 대상에서 제외되지 않습니다. 중복 로드를 피하려면 ",
    "Publicar mediante MCP no excluye las habilidades del descubrimiento predeterminado de Codex. Para evitar cargarlas dos veces, ",
    "La publication via MCP ne retire pas les compétences de la découverte par défaut de Codex. Pour éviter un double chargement, ",
    "MCP-Veröffentlichung entfernt Skills nicht aus der Standardsuche von Codex. Um doppeltes Laden zu vermeiden, ",
    "Publicar pelo MCP não remove habilidades da descoberta padrão do Codex. Para evitar carregamento duplo, "
  ],
  "MCP公開はCodexの標準探索を変更しません。直接読み込みとの重複を避けるには、": [
    "MCP publication does not change Codex’s default discovery. To avoid duplicate direct loading, ",
    "MCP 公开不会更改 Codex 的默认发现。为避免与直接读取重复，请",
    "MCP 公開不會變更 Codex 的預設探索。為避免與直接讀取重複，請",
    "MCP 공개는 Codex의 기본 탐색을 변경하지 않습니다. 직접 읽기와의 중복을 피하려면 ",
    "La publicación MCP no cambia el descubrimiento predeterminado de Codex. Para evitar lecturas duplicadas, ",
    "La publication MCP ne modifie pas la découverte par défaut de Codex. Pour éviter les lectures en double, ",
    "MCP-Veröffentlichung ändert die Standardsuche von Codex nicht. Um doppelte direkte Zugriffe zu vermeiden, ",
    "A publicação MCP não altera a descoberta padrão do Codex. Para evitar leituras duplicadas, "
  ],
  "ソース画面で専用フォルダーへ移動": [
    "move to a dedicated folder on the Folders page",
    "在文件夹页面移至专用文件夹",
    "在資料夾頁面移至專用資料夾",
    "폴더 화면에서 전용 폴더로 이동",
    "mueve las habilidades a una carpeta exclusiva desde Carpetas",
    "déplacez-les vers un dossier dédié depuis Dossiers",
    "auf der Ordnerseite in einen eigenen Ordner verschieben",
    "mova para uma pasta exclusiva na página Pastas"
  ],
  "現在のファイルは読み取れます。最後のスキャン時は取得できなかったため、MCPの索引は復旧待ちです。": [
    "The file is readable now, but the last scan failed. The MCP index needs a rescan. Please ",
    "文件现在可读取，但上次扫描失败。MCP 索引需要重新扫描。请",
    "檔案現在可讀取，但上次掃描失敗。MCP 索引需要重新掃描。請",
    "현재 파일을 읽을 수 있지만 마지막 스캔에 실패했습니다. MCP 색인을 복구하려면 ",
    "Ahora se puede leer el archivo, pero falló el último escaneo. El índice MCP necesita otro escaneo. Puedes ",
    "Le fichier est lisible, mais la dernière analyse a échoué. L’index MCP nécessite une nouvelle analyse. Veuillez ",
    "Die Datei ist jetzt lesbar, aber der letzte Scan schlug fehl. Der MCP-Index benötigt einen neuen Scan. Bitte ",
    "O arquivo pode ser lido agora, mas a última verificação falhou. O índice MCP precisa de nova verificação. Você pode "
  ],
  "元の SKILL.md が見つかりません。表示中の本文は最後にスキャンした保存内容です。元のフォルダーが存在する場合は本文を再作成できます。ソースのパスも確認してください。": [
    "The original SKILL.md is missing. The last scanned content is shown. You can recreate it if its folder exists. Also check the source path.",
    "找不到原始 SKILL.md。当前显示的是上次扫描的内容。若原文件夹存在，可重新创建正文。也请检查来源路径。",
    "找不到原始 SKILL.md。目前顯示上次掃描的內容。若原資料夾存在，可重新建立內容。也請檢查來源路徑。",
    "원본 SKILL.md가 없습니다. 마지막 스캔 내용이 표시됩니다. 원본 폴더가 있으면 본문을 다시 만들 수 있습니다. 소스 경로도 확인하세요.",
    "No se encuentra el SKILL.md original. Se muestra el último contenido escaneado. Puedes recrearlo si su carpeta existe. Revisa también la ruta de origen.",
    "Le SKILL.md original est introuvable. Le dernier contenu analysé est affiché. Vous pouvez le recréer si son dossier existe. Vérifiez aussi le chemin source.",
    "Die ursprüngliche SKILL.md fehlt. Der zuletzt gescannte Inhalt wird angezeigt. Ist der Ordner vorhanden, kann der Inhalt neu erstellt werden. Prüfen Sie auch den Quellpfad.",
    "O SKILL.md original não foi encontrado. O último conteúdo verificado é exibido. Você pode recriá-lo se a pasta existir. Verifique também o caminho de origem."
  ],
  "同じ名前のスキルがあります。ソースとパスを確認してください。": [
    "Another skill has the same name. Check its source and path.",
    "存在同名技能。请检查来源和路径。",
    "存在同名技能。請檢查來源與路徑。",
    "같은 이름의 스킬이 있습니다. 소스와 경로를 확인하세요.",
    "Hay otra habilidad con el mismo nombre. Revisa su origen y ruta.",
    "Une autre compétence porte le même nom. Vérifiez sa source et son chemin.",
    "Ein anderer Skill hat denselben Namen. Prüfen Sie Quelle und Pfad.",
    "Outra habilidade tem o mesmo nome. Verifique a origem e o caminho."
  ],
  "安全のため、HTML・画像・リンクの遷移は無効です。": [
    "For safety, HTML, images and link navigation are disabled.",
    "为确保安全，已禁用 HTML、图片和链接跳转。",
    "為確保安全，已停用 HTML、圖片及連結跳轉。",
    "안전을 위해 HTML, 이미지 및 링크 이동이 비활성화됩니다.",
    "Por seguridad, se desactivan HTML, imágenes y navegación por enlaces.",
    "Par sécurité, HTML, images et navigation par liens sont désactivés.",
    "Aus Sicherheitsgründen sind HTML, Bilder und Linknavigation deaktiviert.",
    "Por segurança, HTML, imagens e navegação por links estão desativados."
  ],
  "SKILL.md と、このフォルダー以下の関連ファイルが MCP の取得対象になります。関連ファイル一覧を確認してください。ファイル不在や読み取りエラー時は、公開設定にかかわらず対象外です。": [
    "MCP can retrieve SKILL.md and related files beneath this folder. Review the related files. Missing or unreadable files are excluded regardless of publication settings.",
    "MCP 可获取 SKILL.md 及此文件夹下的相关文件。请检查相关文件列表。文件不存在或无法读取时，不论公开设置如何都不会提供。",
    "MCP 可取得 SKILL.md 及此資料夾下的相關檔案。請檢查相關檔案清單。檔案不存在或無法讀取時，不論公開設定如何都不會提供。",
    "MCP에서 SKILL.md와 이 폴더 아래의 관련 파일을 조회할 수 있습니다. 관련 파일 목록을 확인하세요. 파일이 없거나 읽기 오류가 있으면 공개 설정과 관계없이 제외됩니다.",
    "MCP puede obtener SKILL.md y los archivos relacionados de esta carpeta. Revisa la lista. Los archivos ausentes o ilegibles quedan excluidos independientemente de la publicación.",
    "MCP peut récupérer SKILL.md et les fichiers associés de ce dossier. Vérifiez la liste. Les fichiers absents ou illisibles sont exclus, quel que soit le réglage de publication.",
    "MCP kann SKILL.md und zugehörige Dateien unter diesem Ordner abrufen. Prüfen Sie die Dateiliste. Fehlende oder unlesbare Dateien werden unabhängig von der Veröffentlichung ausgeschlossen.",
    "O MCP pode obter SKILL.md e arquivos relacionados desta pasta. Confira a lista. Arquivos ausentes ou ilegíveis são excluídos independentemente da publicação."
  ],
  "この履歴の本文に復元しますか？ 現在の本文も履歴に残ります。": [
    "Restore the content from this history entry? The current content will also remain in history.",
    "要恢复到此历史版本吗？当前正文也会保留在历史中。",
    "要還原此歷史版本嗎？目前內容也會保留於歷史紀錄。",
    "이 이력의 본문으로 복원할까요? 현재 본문도 이력에 남습니다.",
    "¿Restaurar el contenido de esta versión? El contenido actual también quedará en el historial.",
    "Restaurer ce contenu ? Le contenu actuel sera également conservé dans l’historique.",
    "Inhalt aus diesem Verlauf wiederherstellen? Der aktuelle Inhalt bleibt ebenfalls im Verlauf.",
    "Restaurar o conteúdo desta versão? O conteúdo atual também ficará no histórico."
  ],
  "指定したソース内にフォルダーと SKILL.md を作成します。既存のファイルは上書きしません。": [
    "Creates a folder and SKILL.md inside the selected source without overwriting existing files.",
    "在指定来源中创建文件夹和 SKILL.md，不会覆盖现有文件。",
    "在指定來源中建立資料夾及 SKILL.md，不會覆寫現有檔案。",
    "선택한 소스 안에 폴더와 SKILL.md를 만듭니다. 기존 파일은 덮어쓰지 않습니다.",
    "Crea una carpeta y SKILL.md en el origen elegido sin sobrescribir archivos existentes.",
    "Crée un dossier et SKILL.md dans la source sélectionnée sans écraser les fichiers existants.",
    "Erstellt einen Ordner und SKILL.md in der gewählten Quelle, ohne bestehende Dateien zu überschreiben.",
    "Cria uma pasta e SKILL.md na origem selecionada sem sobrescrever arquivos existentes."
  ],
  "先に上の接続先をMCPクライアントに登録し、接続を確認してください。": [
    "First add the endpoint above to your MCP client and verify the connection.",
    "请先将上述端点添加到 MCP 客户端并确认连接。",
    "請先將上述端點新增至 MCP 用戶端並確認連線。",
    "먼저 위 주소를 MCP 클라이언트에 등록하고 연결을 확인하세요.",
    "Primero añade la dirección anterior al cliente MCP y comprueba la conexión.",
    "Ajoutez d’abord l’adresse ci-dessus à votre client MCP et vérifiez la connexion.",
    "Fügen Sie zuerst den obigen Endpunkt zum MCP-Client hinzu und prüfen Sie die Verbindung.",
    "Primeiro adicione o endereço acima ao cliente MCP e confirme a conexão."
  ],
  "移動先は同じボリューム内の、まだ存在しないフォルダーを指定します。推奨先は必要なら作成します。任意の移動先の親フォルダーは事前に用意してください。": [
    "Choose a folder that does not yet exist on the same volume. The recommended parent location is created if needed. For a custom destination, create its parent folder first.",
    "请选择同一卷内尚不存在的文件夹。必要时会创建建议位置。自定义目标的父文件夹需预先创建。",
    "請選擇同一磁碟區內尚不存在的資料夾。必要時會建立建議位置。自訂目標的父資料夾需預先建立。",
    "같은 볼륨에서 아직 존재하지 않는 폴더를 지정하세요. 권장 위치는 필요하면 생성합니다. 사용자 지정 대상의 상위 폴더는 미리 만드세요.",
    "Elige una carpeta que aún no exista en el mismo volumen. Se creará la ubicación recomendada si hace falta. Para un destino personalizado, crea primero la carpeta superior.",
    "Choisissez un dossier inexistant sur le même volume. L’emplacement recommandé est créé si nécessaire. Pour une destination personnalisée, créez d’abord le dossier parent.",
    "Wählen Sie einen noch nicht vorhandenen Ordner auf demselben Volume. Der empfohlene Ort wird bei Bedarf erstellt. Bei eigenen Zielen muss der übergeordnete Ordner bereits existieren.",
    "Escolha uma pasta ainda inexistente no mesmo volume. O local recomendado será criado se necessário. Para um destino personalizado, crie primeiro a pasta pai."
  ],
  "Git worktreeやリンクを含むフォルダーは移動できません。ファイル内の絶対パスは書き換えません。": [
    "Folders containing Git worktrees or links cannot be moved. Absolute paths inside files are not rewritten.",
    "无法移动含 Git 工作树或链接的文件夹。不会重写文件内的绝对路径。",
    "無法移動含 Git 工作樹或連結的資料夾。不會改寫檔案內的絕對路徑。",
    "Git 작업 트리나 링크가 포함된 폴더는 이동할 수 없습니다. 파일 안의 절대 경로는 수정하지 않습니다.",
    "No se pueden mover carpetas con worktrees de Git o enlaces. No se reescriben las rutas absolutas dentro de los archivos.",
    "Les dossiers contenant des worktrees Git ou des liens ne peuvent pas être déplacés. Les chemins absolus dans les fichiers ne sont pas réécrits.",
    "Ordner mit Git-Worktrees oder Verknüpfungen können nicht verschoben werden. Absolute Pfade in Dateien werden nicht geändert.",
    "Pastas com worktrees do Git ou links não podem ser movidas. Caminhos absolutos nos arquivos não são reescritos."
  ],
  "移動後はCodexを再起動してください。既存の会話に読み込まれた内容は消えません。以後はMCPでスキルを検索・取得します。": [
    "Restart Codex after moving. Content already loaded into conversations remains. Future skill searches and retrieval use MCP.",
    "移动后请重启 Codex。已载入现有对话的内容不会消失。之后通过 MCP 搜索和获取技能。",
    "移動後請重新啟動 Codex。已載入現有對話的內容不會消失。之後透過 MCP 搜尋和取得技能。",
    "이동 후 Codex를 다시 시작하세요. 기존 대화에 로드된 내용은 남습니다. 이후 스킬 검색과 조회는 MCP를 사용합니다.",
    "Reinicia Codex tras mover. El contenido ya cargado en conversaciones permanece. Las siguientes búsquedas y lecturas usarán MCP.",
    "Redémarrez Codex après le déplacement. Le contenu déjà chargé dans les conversations reste présent. Les recherches et récupérations suivantes passent par MCP.",
    "Starten Sie Codex nach dem Verschieben neu. Bereits in Gespräche geladene Inhalte bleiben erhalten. Zukünftige Skillsuche und Abrufe nutzen MCP.",
    "Reinicie o Codex após mover. O conteúdo já carregado nas conversas permanece. As próximas buscas e leituras usarão MCP."
  ],
  "管理中のフォルダーはありません。下の「フォルダーを追加」から登録してください。": [
    "No managed folders. Use “Add folder” below to register one.",
    "没有管理中的文件夹。请使用下方的“添加文件夹”注册。",
    "沒有管理中的資料夾。請使用下方的「新增資料夾」註冊。",
    "관리 중인 폴더가 없습니다. 아래의 “폴더 추가”로 등록하세요.",
    "No hay carpetas gestionadas. Usa «Añadir carpeta» más abajo.",
    "Aucun dossier géré. Utilisez « Ajouter un dossier » ci-dessous.",
    "Keine verwalteten Ordner. Nutzen Sie unten „Ordner hinzufügen“.",
    "Não há pastas gerenciadas. Use “Adicionar pasta” abaixo."
  ],
  "Codexの探索先かどうか確認できないため、移動できません。フォルダーやドライブへの接続・読み取り権限を確認してください。": [
    "Cannot move because Codex discovery status is unknown. Check the folder or drive connection and read permissions.",
    "无法确认 Codex 的发现状态，因此不能移动。请检查文件夹或驱动器的连接及读取权限。",
    "無法確認 Codex 的探索狀態，因此不能移動。請檢查資料夾或磁碟機的連線及讀取權限。",
    "Codex 탐색 대상 여부를 확인할 수 없어 이동할 수 없습니다. 폴더나 드라이브 연결 및 읽기 권한을 확인하세요.",
    "No se puede mover porque se desconoce si Codex lo explora. Comprueba la conexión de la carpeta o unidad y los permisos de lectura.",
    "Déplacement impossible car l’état de découverte de Codex est inconnu. Vérifiez la connexion au dossier ou lecteur et les droits de lecture.",
    "Verschieben nicht möglich, da der Codex-Suchstatus unbekannt ist. Prüfen Sie Ordner- oder Laufwerksverbindung und Leserechte.",
    "Não é possível mover porque o status de descoberta do Codex é desconhecido. Verifique a conexão da pasta ou unidade e as permissões de leitura."
  ],
  "Codexもこのフォルダーを直接読み込みます。MCPだけで使うには、下の保存先へ移動してください。": [
    "Codex also reads this folder directly. Move it to the location below to use MCP exclusively.",
    "Codex 也会直接读取此文件夹。若只使用 MCP，请移至下方位置。",
    "Codex 也會直接讀取此資料夾。若只使用 MCP，請移至下方位置。",
    "Codex도 이 폴더를 직접 읽습니다. MCP만 사용하려면 아래 위치로 이동하세요.",
    "Codex también lee esta carpeta directamente. Muévela a la ubicación siguiente para usar solo MCP.",
    "Codex lit aussi ce dossier directement. Déplacez-le ci-dessous pour utiliser uniquement MCP.",
    "Codex liest diesen Ordner auch direkt. Verschieben Sie ihn an den folgenden Ort, um ausschließlich MCP zu nutzen.",
    "O Codex também lê esta pasta diretamente. Mova-a para o local abaixo para usar apenas MCP."
  ],
  "Codexの標準探索先の外です。独自の探索設定がある場合は、そちらも確認してください。": [
    "Outside Codex’s default discovery locations. Check any custom discovery settings as well.",
    "不在 Codex 默认发现范围内。如有自定义发现设置，也请检查。",
    "不在 Codex 預設探索範圍內。如有自訂探索設定，也請檢查。",
    "Codex의 기본 탐색 위치 밖입니다. 사용자 지정 탐색 설정도 확인하세요.",
    "Está fuera del descubrimiento predeterminado de Codex. Revisa también la configuración personalizada.",
    "Hors des emplacements de découverte par défaut de Codex. Vérifiez aussi vos réglages personnalisés.",
    "Außerhalb der Standardsuchorte von Codex. Prüfen Sie auch eigene Sucheinstellungen.",
    "Fora dos locais padrão de descoberta do Codex. Verifique também as configurações personalizadas."
  ],
  "このフォルダーを管理から外しますか？ フォルダーとファイルは削除しません。既定のフォルダーの場合は自動登録も停止します。": [
    "Stop managing this folder? The folder and files will remain. For the default folder, automatic registration will also stop.",
    "要停止管理此文件夹吗？不会删除文件夹和文件。若是默认文件夹，也会停止自动注册。",
    "要停止管理此資料夾嗎？不會刪除資料夾和檔案。若為預設資料夾，也會停止自動註冊。",
    "이 폴더를 관리에서 제외할까요? 폴더와 파일은 유지됩니다. 기본 폴더는 자동 등록도 중단됩니다.",
    "¿Dejar de gestionar esta carpeta? La carpeta y los archivos se conservarán. Si es la predeterminada, también se detendrá el registro automático.",
    "Ne plus gérer ce dossier ? Le dossier et les fichiers restent présents. Pour le dossier par défaut, l’enregistrement automatique sera aussi arrêté.",
    "Verwaltung dieses Ordners beenden? Ordner und Dateien bleiben erhalten. Beim Standardordner wird auch die automatische Registrierung beendet.",
    "Parar de gerenciar esta pasta? A pasta e os arquivos permanecerão. Para a pasta padrão, o registro automático também será interrompido."
  ],
  "既存のフォルダーを追加して、すべての階層のスキルを読み込みます。ファイルは移動せず、新しいスキルは非公開で登録します。": [
    "Add an existing folder to load skills from all levels. Files stay in place; new skills are registered as unpublished.",
    "添加现有文件夹以读取所有层级的技能。不移动文件，新技能注册为未公开。",
    "新增現有資料夾以讀取所有層級的技能。不移動檔案，新技能註冊為未公開。",
    "기존 폴더를 추가해 모든 하위 단계의 스킬을 읽습니다. 파일은 이동하지 않으며 새 스킬은 비공개로 등록됩니다.",
    "Añade una carpeta existente para cargar habilidades de todos los niveles. Los archivos no se mueven; las nuevas habilidades se registran sin publicar.",
    "Ajoutez un dossier existant pour charger les compétences à tous les niveaux. Les fichiers restent en place ; les nouvelles compétences ne sont pas publiées.",
    "Fügen Sie einen bestehenden Ordner hinzu, um Skills aller Ebenen zu laden. Dateien bleiben unverändert; neue Skills sind zunächst nicht veröffentlicht.",
    "Adicione uma pasta existente para carregar habilidades de todos os níveis. Os arquivos permanecem no lugar; novas habilidades não são publicadas."
  ],
  "既定のフォルダーの自動登録は停止中です。指定して保存すると管理を再開します。": [
    "Automatic registration of the default folder is paused. Specify and save it to resume management.",
    "默认文件夹的自动注册已停止。指定并保存后可恢复管理。",
    "預設資料夾的自動註冊已停止。指定並儲存後可恢復管理。",
    "기본 폴더의 자동 등록이 중단되었습니다. 지정하고 저장하면 관리를 재개합니다.",
    "El registro automático de la carpeta predeterminada está detenido. Indícala y guarda para reanudarlo.",
    "L’enregistrement automatique du dossier par défaut est arrêté. Indiquez-le et enregistrez pour reprendre.",
    "Die automatische Registrierung des Standardordners ist angehalten. Geben Sie ihn an und speichern Sie, um fortzufahren.",
    "O registro automático da pasta padrão está interrompido. Indique e salve para retomar."
  ],
  "別のフォルダーを管理対象に切り替えます。スキルのファイルは移動しません。": [
    "Switch management to a different folder. Skill files will not be moved.",
    "切换到管理另一个文件夹。不会移动技能文件。",
    "切換為管理另一個資料夾。不會移動技能檔案。",
    "다른 폴더를 관리 대상으로 전환합니다. 스킬 파일은 이동하지 않습니다.",
    "Cambia la gestión a otra carpeta. Los archivos de habilidades no se moverán.",
    "Gérez un autre dossier. Les fichiers des compétences ne seront pas déplacés.",
    "Wechselt die Verwaltung zu einem anderen Ordner. Skilldateien werden nicht verschoben.",
    "Muda o gerenciamento para outra pasta. Os arquivos das habilidades não serão movidos."
  ],
  "元のフォルダーを移動先へ丸ごと移動し、有効なスキルをすべてMCP公開します。元の場所には残りません。MCP接続を確認したうえで実行しますか？": [
    "Move the entire original folder and publish all valid skills via MCP? Nothing remains at the old location. Confirm the MCP connection before continuing.",
    "要移动整个原文件夹并通过 MCP 公开所有有效技能吗？原位置不会保留。请先确认 MCP 连接。",
    "要移動整個原資料夾並透過 MCP 公開所有有效技能嗎？原位置不會保留。請先確認 MCP 連線。",
    "원본 폴더 전체를 이동하고 유효한 모든 스킬을 MCP에 공개할까요? 원래 위치에는 남지 않습니다. 먼저 MCP 연결을 확인하세요.",
    "¿Mover toda la carpeta original y publicar todas las habilidades válidas mediante MCP? No quedará nada en la ubicación anterior. Confirma primero la conexión MCP.",
    "Déplacer tout le dossier et publier toutes les compétences valides via MCP ? Rien ne restera à l’ancien emplacement. Vérifiez d’abord la connexion MCP.",
    "Gesamten ursprünglichen Ordner verschieben und alle gültigen Skills über MCP veröffentlichen? Am alten Ort bleibt nichts zurück. Prüfen Sie zuerst die MCP-Verbindung.",
    "Mover toda a pasta original e publicar todas as habilidades válidas pelo MCP? Nada ficará no local antigo. Confirme a conexão MCP antes de continuar."
  ],
  "元のフォルダーを丸ごと移動し、有効な": [
    "Move the entire original folder and publish ",
    "移动整个原文件夹，并公开有效技能 ",
    "移動整個原資料夾，並公開有效技能 ",
    "원본 폴더 전체를 이동하고 유효한 스킬 ",
    "Mover toda la carpeta original y publicar ",
    "Déplacer tout le dossier et publier ",
    "Gesamten ursprünglichen Ordner verschieben und gültige Skills veröffentlichen: ",
    "Mover toda a pasta original e publicar "
  ],
  "件をMCP公開します。移動後はCodexを再起動してください。": [
    " valid skills via MCP. Restart Codex after moving.",
    " 项到 MCP。移动后请重启 Codex。",
    " 項至 MCP。移動後請重新啟動 Codex。",
    "개를 MCP에 공개합니다. 이동 후 Codex를 다시 시작하세요.",
    " habilidades válidas mediante MCP. Reinicia Codex después.",
    " compétences valides via MCP. Redémarrez Codex ensuite.",
    " über MCP. Starten Sie Codex anschließend neu.",
    " habilidades válidas pelo MCP. Reinicie o Codex após mover."
  ],
  "編集した本文と保存前のリビジョンを保持しています。先に本文をコピーし、フォルダーの接続・読み取り権限を確認してください。": [
    "Your edited content and original revision are retained. Copy the content first, then check the folder connection and read permissions.",
    "已保留编辑的正文和保存前的版本。请先复制正文，再检查文件夹连接和读取权限。",
    "已保留編輯的內容與儲存前的版本。請先複製內容，再檢查資料夾連線與讀取權限。",
    "편집한 본문과 저장 전 리비전이 유지됩니다. 먼저 본문을 복사한 후 폴더 연결 및 읽기 권한을 확인하세요.",
    "Se conservan el contenido editado y la revisión original. Copia primero el contenido y revisa la conexión y los permisos.",
    "Votre contenu modifié et la révision d’origine sont conservés. Copiez le contenu, puis vérifiez la connexion et les droits de lecture.",
    "Bearbeiteter Inhalt und ursprüngliche Revision bleiben erhalten. Kopieren Sie den Inhalt und prüfen Sie Verbindung und Leserechte.",
    "O conteúdo editado e a revisão original foram mantidos. Copie o conteúdo e verifique a conexão e as permissões de leitura."
  ],
  "編集中の場合はブラウザーの「戻る」で入力内容を確認してください。": [
    "If you were editing, use your browser’s Back button to check your input.",
    "若正在编辑，请使用浏览器的“返回”检查输入。",
    "若正在編輯，請使用瀏覽器的「上一頁」檢查輸入。",
    "편집 중이었다면 브라우저의 뒤로 가기로 입력 내용을 확인하세요.",
    "Si estabas editando, usa Atrás en el navegador para revisar lo introducido.",
    "Si vous étiez en train de modifier, utilisez Retour dans le navigateur pour vérifier votre saisie.",
    "Wenn Sie Inhalte bearbeitet haben, prüfen Sie Ihre Eingaben über die Zurück-Schaltfläche des Browsers.",
    "Se estava editando, use Voltar no navegador para conferir os dados."
  ],
  "未保存の本文があります。変更を破棄してページを移動しますか？": [
    "You have unsaved content. Discard changes and leave this page?",
    "正文尚未保存。要放弃修改并离开此页面吗？",
    "內容尚未儲存。要捨棄變更並離開此頁面嗎？",
    "저장되지 않은 본문이 있습니다. 변경 사항을 버리고 이동할까요?",
    "Hay contenido sin guardar. ¿Descartar los cambios y salir?",
    "Le contenu n’est pas enregistré. Abandonner les modifications et quitter la page ?",
    "Es gibt ungespeicherte Inhalte. Änderungen verwerfen und die Seite verlassen?",
    "Há conteúdo não salvo. Descartar as alterações e sair desta página?"
  ],
  "未保存の本文があります。変更を破棄して操作を続けますか？": [
    "You have unsaved content. Discard changes and continue?",
    "正文尚未保存。要放弃修改并继续吗？",
    "內容尚未儲存。要捨棄變更並繼續嗎？",
    "저장되지 않은 본문이 있습니다. 변경 사항을 버리고 계속할까요?",
    "Hay contenido sin guardar. ¿Descartar los cambios y continuar?",
    "Le contenu n’est pas enregistré. Abandonner les modifications et continuer ?",
    "Es gibt ungespeicherte Inhalte. Änderungen verwerfen und fortfahren?",
    "Há conteúdo não salvo. Descartar as alterações e continuar?"
  ],
  "プレビューを表示できませんでした。本文は編集欄に保持されています。": [
    "Could not display the preview. Your content remains in the editor.",
    "无法显示预览。正文仍保留在编辑栏。",
    "無法顯示預覽。內容仍保留於編輯欄。",
    "미리 보기를 표시하지 못했습니다. 본문은 편집기에 유지됩니다.",
    "No se pudo mostrar la vista previa. El contenido permanece en el editor.",
    "Impossible d’afficher l’aperçu. Le contenu reste dans l’éditeur.",
    "Die Vorschau konnte nicht angezeigt werden. Der Inhalt bleibt im Editor erhalten.",
    "Não foi possível exibir a prévia. O conteúdo permanece no editor."
  ],
  "表示設定はこのブラウザーに保存されます。": [
    "Display preferences are saved in this browser.",
    "显示设置保存在此浏览器中。",
    "顯示設定儲存於此瀏覽器。",
    "표시 설정은 이 브라우저에 저장됩니다.",
    "Las preferencias se guardan en este navegador.",
    "Les préférences sont enregistrées dans ce navigateur.",
    "Anzeigeeinstellungen werden in diesem Browser gespeichert.",
    "As preferências são salvas neste navegador."
  ],
  "設定を保存できませんでした。このタブには適用されています。": [
    "Could not save preferences. They are applied to this tab.",
    "无法保存设置。已应用于此标签页。",
    "無法儲存設定。已套用至此分頁。",
    "설정을 저장하지 못했습니다. 현재 탭에는 적용되었습니다.",
    "No se pudieron guardar las preferencias. Se aplican a esta pestaña.",
    "Impossible d’enregistrer les préférences. Elles sont appliquées à cet onglet.",
    "Einstellungen konnten nicht gespeichert werden. Sie gelten für diesen Tab.",
    "Não foi possível salvar as preferências. Elas se aplicam a esta aba."
  ],
  "表示設定と更新の確認にはJavaScriptが必要です。": [
    "Display settings and update checks require JavaScript.",
    "显示设置和更新检查需要 JavaScript。",
    "顯示設定與更新檢查需要 JavaScript。",
    "표시 설정과 업데이트 확인에는 JavaScript가 필요합니다.",
    "Los ajustes de apariencia y las actualizaciones requieren JavaScript.",
    "Les réglages d’affichage et la recherche de mises à jour nécessitent JavaScript.",
    "Anzeigeeinstellungen und Updateprüfungen benötigen JavaScript.",
    "As configurações de aparência e a busca de atualizações exigem JavaScript."
  ],
  "アプリの更新": [
    "App updates",
    "应用更新",
    "應用程式更新",
    "앱 업데이트",
    "Actualizaciones de la aplicación",
    "Mises à jour de l’application",
    "App-Updates",
    "Atualizações do aplicativo"
  ],
  "現在のバージョン:": [
    "Current version:",
    "当前版本：",
    "目前版本：",
    "현재 버전:",
    "Versión actual:",
    "Version actuelle :",
    "Aktuelle Version:",
    "Versão atual:"
  ],
  "更新を確認できます。": [
    "Ready to check for updates.",
    "可以检查更新。",
    "可以檢查更新。",
    "업데이트를 확인할 수 있습니다.",
    "Puedes buscar actualizaciones.",
    "Vous pouvez rechercher des mises à jour.",
    "Bereit zur Suche nach Updates.",
    "Pronto para buscar atualizações."
  ],
  "更新を確認": [
    "Check for updates",
    "检查更新",
    "檢查更新",
    "업데이트 확인",
    "Buscar actualizaciones",
    "Rechercher des mises à jour",
    "Nach Updates suchen",
    "Buscar atualizações"
  ],
  "更新をインストール": [
    "Install update",
    "安装更新",
    "安裝更新",
    "업데이트 설치",
    "Instalar actualización",
    "Installer la mise à jour",
    "Update installieren",
    "Instalar atualização"
  ],
  "リリースを開く": [
    "Open release",
    "打开发布页面",
    "開啟版本頁面",
    "릴리스 열기",
    "Abrir versión",
    "Ouvrir la version",
    "Release öffnen",
    "Abrir versão"
  ],
  "更新をインストールするとアプリを再起動します。編集中の内容を保存してから実行してください。": [
    "Installing an update restarts the app. Save your edits before continuing.",
    "安装更新会重启应用。请先保存正在编辑的内容。",
    "安裝更新會重新啟動應用程式。請先儲存編輯中的內容。",
    "업데이트를 설치하면 앱이 다시 시작됩니다. 먼저 편집 내용을 저장하세요.",
    "La instalación reinicia la aplicación. Guarda los cambios antes de continuar.",
    "L’installation redémarre l’application. Enregistrez vos modifications avant de continuer.",
    "Das Update startet die App neu. Speichern Sie vorher Ihre Änderungen.",
    "A instalação reinicia o aplicativo. Salve suas alterações antes de continuar."
  ],
  "更新を確認中…": [
    "Checking for updates…",
    "正在检查更新…",
    "正在檢查更新…",
    "업데이트 확인 중…",
    "Buscando actualizaciones…",
    "Recherche de mises à jour…",
    "Updates werden gesucht…",
    "Buscando atualizações…"
  ],
  "最新バージョンです。": [
    "You are up to date.",
    "已是最新版本。",
    "已是最新版本。",
    "최신 버전입니다.",
    "La aplicación está actualizada.",
    "L’application est à jour.",
    "Sie verwenden die aktuelle Version.",
    "O aplicativo está atualizado."
  ],
  "バージョン {version} を利用できます。": [
    "Version {version} is available.",
    "版本 {version} 可用。",
    "版本 {version} 可用。",
    "버전 {version}을 사용할 수 있습니다.",
    "La versión {version} está disponible.",
    "La version {version} est disponible.",
    "Version {version} ist verfügbar.",
    "A versão {version} está disponível."
  ],
  "この環境では自動更新を利用できません。リリースページを確認してください。": [
    "Automatic updates are unavailable on this platform. Visit the release page.",
    "此环境不支持自动更新。请查看发布页面。",
    "此環境不支援自動更新。請查看版本頁面。",
    "이 환경에서는 자동 업데이트를 사용할 수 없습니다. 릴리스 페이지를 확인하세요.",
    "Las actualizaciones automáticas no están disponibles en esta plataforma. Consulta la página de versiones.",
    "Les mises à jour automatiques ne sont pas disponibles sur cette plateforme. Consultez la page des versions.",
    "Automatische Updates sind auf dieser Plattform nicht verfügbar. Öffnen Sie die Releaseseite.",
    "As atualizações automáticas não estão disponíveis nesta plataforma. Consulte a página de versões."
  ],
  "更新の確認に失敗しました。接続を確認して再試行してください。": [
    "Could not check for updates. Check your connection and try again.",
    "检查更新失败。请检查连接后重试。",
    "檢查更新失敗。請檢查連線後重試。",
    "업데이트를 확인하지 못했습니다. 연결을 확인하고 다시 시도하세요.",
    "No se pudieron buscar actualizaciones. Revisa la conexión e inténtalo de nuevo.",
    "Impossible de rechercher les mises à jour. Vérifiez votre connexion et réessayez.",
    "Updatesuche fehlgeschlagen. Prüfen Sie die Verbindung und versuchen Sie es erneut.",
    "Não foi possível buscar atualizações. Verifique a conexão e tente novamente."
  ],
  "更新のインストールを開始しますか？ アプリを再起動します。": [
    "Start installing the update? The app will restart.",
    "开始安装更新吗？应用将重启。",
    "開始安裝更新嗎？應用程式將重新啟動。",
    "업데이트 설치를 시작할까요? 앱이 다시 시작됩니다.",
    "¿Instalar la actualización? La aplicación se reiniciará.",
    "Installer la mise à jour ? L’application redémarrera.",
    "Update installieren? Die App wird neu gestartet.",
    "Instalar a atualização? O aplicativo será reiniciado."
  ],
  "更新をダウンロードして検証中…": [
    "Downloading and verifying the update…",
    "正在下载并验证更新…",
    "正在下載並驗證更新…",
    "업데이트 다운로드 및 검증 중…",
    "Descargando y verificando la actualización…",
    "Téléchargement et vérification de la mise à jour…",
    "Update wird heruntergeladen und geprüft…",
    "Baixando e verificando a atualização…"
  ],
  "更新を開始しました。アプリの再起動をお待ちください。": [
    "Update started. Wait for the app to restart.",
    "更新已开始。请等待应用重启。",
    "更新已開始。請等待應用程式重新啟動。",
    "업데이트가 시작되었습니다. 앱이 다시 시작될 때까지 기다리세요.",
    "Actualización iniciada. Espera a que la aplicación se reinicie.",
    "Mise à jour démarrée. Attendez le redémarrage de l’application.",
    "Update gestartet. Warten Sie, bis die App neu startet.",
    "Atualização iniciada. Aguarde o reinício do aplicativo."
  ],
  "更新をインストールできませんでした。再度更新を確認してください。": [
    "Could not install the update. Check for updates again.",
    "无法安装更新。请再次检查更新。",
    "無法安裝更新。請再次檢查更新。",
    "업데이트를 설치하지 못했습니다. 업데이트를 다시 확인하세요.",
    "No se pudo instalar la actualización. Vuelve a buscar actualizaciones.",
    "Impossible d’installer la mise à jour. Relancez la recherche.",
    "Update konnte nicht installiert werden. Suchen Sie erneut nach Updates.",
    "Não foi possível instalar a atualização. Busque atualizações novamente."
  ],
  "画面を表示できませんでした。再読み込みしてください。": [
    "Could not display this page. Please reload.",
    "无法显示页面。请重新加载。",
    "無法顯示頁面。請重新載入。",
    "화면을 표시하지 못했습니다. 새로 고침하세요.",
    "No se pudo mostrar la página. Vuelve a cargarla.",
    "Impossible d’afficher la page. Rechargez-la.",
    "Die Seite konnte nicht angezeigt werden. Bitte neu laden.",
    "Não foi possível exibir a página. Recarregue-a."
  ],
  "操作用トークンが一致しません。画面を再読み込みしてからやり直してください。": [
    "The operation token does not match. Reload the page and try again.",
    "操作令牌不匹配。请重新加载页面后重试。",
    "操作權杖不符。請重新載入頁面後重試。",
    "작업 토큰이 일치하지 않습니다. 화면을 새로 고침한 후 다시 시도하세요.",
    "El token no coincide. Recarga la página e inténtalo de nuevo.",
    "Le jeton ne correspond pas. Rechargez la page et réessayez.",
    "Das Aktionstoken stimmt nicht überein. Laden Sie die Seite neu und versuchen Sie es erneut.",
    "O token não corresponde. Recarregue a página e tente novamente."
  ],
  "操作できませんでした: {detail}": [
    "Operation failed: {detail}",
    "操作失败：{detail}",
    "操作失敗：{detail}",
    "작업 실패: {detail}",
    "Error en la operación: {detail}",
    "Échec de l’opération : {detail}",
    "Vorgang fehlgeschlagen: {detail}",
    "Falha na operação: {detail}"
  ],
  "登録方法が不正です。": [
    "Invalid registration method.",
    "注册方式无效。",
    "註冊方式無效。",
    "잘못된 등록 방법입니다.",
    "Método de registro no válido.",
    "Méthode d’enregistrement invalide.",
    "Ungültige Registrierungsmethode.",
    "Método de registro inválido."
  ],
  "スキルを1件以上選んでください。ブラウザーでJavaScriptを有効にしてください。": [
    "Select at least one skill. Enable JavaScript in your browser.",
    "请至少选择一个技能。请启用浏览器的 JavaScript。",
    "請至少選取一個技能。請啟用瀏覽器的 JavaScript。",
    "스킬을 하나 이상 선택하세요. 브라우저에서 JavaScript를 활성화하세요.",
    "Selecciona al menos una habilidad. Activa JavaScript en el navegador.",
    "Sélectionnez au moins une compétence. Activez JavaScript dans votre navigateur.",
    "Wählen Sie mindestens einen Skill aus. Aktivieren Sie JavaScript im Browser.",
    "Selecione pelo menos uma habilidade. Ative o JavaScript no navegador."
  ],
  "公開状態が不正です。": [
    "Invalid publication status.",
    "公开状态无效。",
    "公開狀態無效。",
    "잘못된 공개 상태입니다.",
    "Estado de publicación no válido.",
    "État de publication invalide.",
    "Ungültiger Veröffentlichungsstatus.",
    "Status de publicação inválido."
  ],
  "変更する公開状態または追加タグを指定してください。": [
    "Choose a publication change or tags to add.",
    "请选择要更改的公开状态或要添加的标签。",
    "請選擇要變更的公開狀態或要新增的標籤。",
    "변경할 공개 상태 또는 추가할 태그를 지정하세요.",
    "Indica un cambio de publicación o etiquetas para añadir.",
    "Choisissez un changement de publication ou des étiquettes à ajouter.",
    "Wählen Sie einen Veröffentlichungsstatus oder hinzuzufügende Tags.",
    "Escolha uma alteração de publicação ou etiquetas para adicionar."
  ],
  "選択したスキルを更新しました。": [
    "Updated the selected skills.",
    "已更新所选技能。",
    "已更新所選技能。",
    "선택한 스킬을 업데이트했습니다.",
    "Se actualizaron las habilidades seleccionadas.",
    "Les compétences sélectionnées ont été mises à jour.",
    "Die ausgewählten Skills wurden aktualisiert.",
    "As habilidades selecionadas foram atualizadas."
  ],
  "SKILL.md を保存しました。": [
    "Saved SKILL.md.",
    "已保存 SKILL.md。",
    "已儲存 SKILL.md。",
    "SKILL.md를 저장했습니다.",
    "Se guardó SKILL.md.",
    "SKILL.md a été enregistré.",
    "SKILL.md wurde gespeichert.",
    "SKILL.md foi salvo."
  ],
  "管理情報を保存しました。": [
    "Saved metadata.",
    "已保存管理信息。",
    "已儲存管理資訊。",
    "관리 정보를 저장했습니다.",
    "Se guardaron los metadatos.",
    "Les métadonnées ont été enregistrées.",
    "Metadaten wurden gespeichert.",
    "Os metadados foram salvos."
  ],
  "履歴から復元しました。": [
    "Restored from history.",
    "已从历史记录恢复。",
    "已從歷史紀錄還原。",
    "이력에서 복원했습니다.",
    "Se restauró desde el historial.",
    "Restauration depuis l’historique effectuée.",
    "Aus dem Verlauf wiederhergestellt.",
    "Restaurado do histórico."
  ],
  "スキルを作成しました。公開するには管理情報で公開を有効にしてください。": [
    "Skill created. Enable publication in metadata to publish it.",
    "已创建技能。要公开，请在管理信息中启用公开。",
    "已建立技能。若要公開，請在管理資訊中啟用公開。",
    "스킬을 만들었습니다. 공개하려면 관리 정보에서 공개를 활성화하세요.",
    "Habilidad creada. Activa la publicación en los metadatos para publicarla.",
    "Compétence créée. Activez la publication dans les métadonnées pour la publier.",
    "Skill erstellt. Aktivieren Sie die Veröffentlichung in den Metadaten.",
    "Habilidade criada. Ative a publicação nos metadados para publicá-la."
  ],
  "本文は2 MiB以内にしてください。": [
    "Content must not exceed 2 MiB.",
    "正文不能超过 2 MiB。",
    "內容不能超過 2 MiB。",
    "본문은 2 MiB 이하여야 합니다.",
    "El contenido no puede superar 2 MiB.",
    "Le contenu ne doit pas dépasser 2 Mio.",
    "Der Inhalt darf 2 MiB nicht überschreiten.",
    "O conteúdo não pode exceder 2 MiB."
  ],
  "登録を解除しました。元のフォルダーとファイルは保持されています。": [
    "Folder unregistered. The original folder and files remain.",
    "已取消注册。原文件夹和文件已保留。",
    "已取消註冊。原資料夾及檔案已保留。",
    "등록을 해제했습니다. 원본 폴더와 파일은 유지됩니다.",
    "Registro eliminado. La carpeta y los archivos originales se conservan.",
    "Dossier désenregistré. Le dossier et les fichiers d’origine sont conservés.",
    "Ordner abgemeldet. Ursprünglicher Ordner und Dateien bleiben erhalten.",
    "Registro removido. A pasta e os arquivos originais permanecem."
  ],
  "管理対象フォルダーの設定を保存しました。": [
    "Managed folder settings saved.",
    "已保存管理文件夹设置。",
    "已儲存管理資料夾設定。",
    "관리 대상 폴더 설정을 저장했습니다.",
    "Se guardó la configuración de la carpeta gestionada.",
    "Les réglages du dossier géré ont été enregistrés.",
    "Einstellungen des verwalteten Ordners gespeichert.",
    "As configurações da pasta gerenciada foram salvas."
  ],
  "設定を変更できませんでした: {detail}。以前の設定は保持されています。": [
    "Could not change settings: {detail}. Previous settings are retained.",
    "无法更改设置：{detail}。已保留之前的设置。",
    "無法變更設定：{detail}。已保留先前的設定。",
    "설정을 변경하지 못했습니다: {detail}. 이전 설정은 유지됩니다.",
    "No se pudo cambiar la configuración: {detail}. Se conserva la anterior.",
    "Impossible de modifier les réglages : {detail}. Les réglages précédents sont conservés.",
    "Einstellungen konnten nicht geändert werden: {detail}. Vorherige Einstellungen bleiben erhalten.",
    "Não foi possível alterar as configurações: {detail}. As anteriores foram mantidas."
  ],
  "登録できませんでした: {detail}": [
    "Could not register: {detail}",
    "无法注册：{detail}",
    "無法註冊：{detail}",
    "등록하지 못했습니다: {detail}",
    "No se pudo registrar: {detail}",
    "Impossible d’enregistrer : {detail}",
    "Registrierung fehlgeschlagen: {detail}",
    "Não foi possível registrar: {detail}"
  ],
  "復元できませんでした: {detail}": [
    "Could not restore: {detail}",
    "无法恢复：{detail}",
    "無法還原：{detail}",
    "복원하지 못했습니다: {detail}",
    "No se pudo restaurar: {detail}",
    "Impossible de restaurer : {detail}",
    "Wiederherstellung fehlgeschlagen: {detail}",
    "Não foi possível restaurar: {detail}"
  ],
  "作成できませんでした: {detail}": [
    "Could not create: {detail}",
    "无法创建：{detail}",
    "無法建立：{detail}",
    "만들지 못했습니다: {detail}",
    "No se pudo crear: {detail}",
    "Impossible de créer : {detail}",
    "Erstellen fehlgeschlagen: {detail}",
    "Não foi possível criar: {detail}"
  ],
  "スキャン完了: {count}件を登録、エラー{errors}件、見つからないスキル{missing}件。": [
    "Scan complete: {count} registered, {errors} errors, {missing} missing skills.",
    "扫描完成：已注册 {count} 项，{errors} 个错误，{missing} 个缺失技能。",
    "掃描完成：已註冊 {count} 項，{errors} 個錯誤，{missing} 個遺失技能。",
    "스캔 완료: {count}개 등록, 오류 {errors}개, 누락된 스킬 {missing}개.",
    "Escaneo completo: {count} registradas, {errors} errores y {missing} habilidades ausentes.",
    "Analyse terminée : {count} enregistrées, {errors} erreurs, {missing} compétences introuvables.",
    "Scan abgeschlossen: {count} registriert, {errors} Fehler, {missing} fehlende Skills.",
    "Verificação concluída: {count} registradas, {errors} erros, {missing} habilidades ausentes."
  ],
  "ライブラリ · Skilvella": [
    "Library · Skilvella",
    "库 · Skilvella",
    "資料庫 · Skilvella",
    "라이브러리 · Skilvella",
    "Biblioteca · Skilvella",
    "Bibliothèque · Skilvella",
    "Bibliothek · Skilvella",
    "Biblioteca · Skilvella"
  ],
  "フォルダー · Skilvella": [
    "Folders · Skilvella",
    "文件夹 · Skilvella",
    "資料夾 · Skilvella",
    "폴더 · Skilvella",
    "Carpetas · Skilvella",
    "Dossiers · Skilvella",
    "Ordner · Skilvella",
    "Pastas · Skilvella"
  ],
  "新規スキル · Skilvella": [
    "New skill · Skilvella",
    "新建技能 · Skilvella",
    "新增技能 · Skilvella",
    "새 스킬 · Skilvella",
    "Nueva habilidad · Skilvella",
    "Nouvelle compétence · Skilvella",
    "Neuer Skill · Skilvella",
    "Nova habilidade · Skilvella"
  ],
  "未保存の下書き · Skilvella": [
    "Unsaved draft · Skilvella",
    "未保存的草稿 · Skilvella",
    "未儲存的草稿 · Skilvella",
    "저장되지 않은 초안 · Skilvella",
    "Borrador sin guardar · Skilvella",
    "Brouillon non enregistré · Skilvella",
    "Ungespeicherter Entwurf · Skilvella",
    "Rascunho não salvo · Skilvella"
  ],
  "設定 · Skilvella": [
    "Settings · Skilvella",
    "设置 · Skilvella",
    "設定 · Skilvella",
    "설정 · Skilvella",
    "Configuración · Skilvella",
    "Paramètres · Skilvella",
    "Einstellungen · Skilvella",
    "Configurações · Skilvella"
  ],
  "操作エラー · Skilvella": [
    "Error · Skilvella",
    "错误 · Skilvella",
    "錯誤 · Skilvella",
    "오류 · Skilvella",
    "Error · Skilvella",
    "Erreur · Skilvella",
    "Fehler · Skilvella",
    "Erro · Skilvella"
  ],
  "ソースフォルダーを専用フォルダーへ移動し、有効なスキルをMCP公開しました。Codexを再起動してください。": [
    "Moved the source to a dedicated folder and published valid skills via MCP. Restart Codex.",
    "已将来源移至专用文件夹，并通过 MCP 公开有效技能。请重启 Codex。",
    "已將來源移至專用資料夾，並透過 MCP 公開有效技能。請重新啟動 Codex。",
    "소스를 전용 폴더로 이동하고 유효한 스킬을 MCP에 공개했습니다. Codex를 다시 시작하세요.",
    "Se movió el origen a una carpeta exclusiva y se publicaron las habilidades válidas mediante MCP. Reinicia Codex.",
    "La source a été déplacée vers un dossier dédié et les compétences valides ont été publiées via MCP. Redémarrez Codex.",
    "Quelle in einen eigenen Ordner verschoben und gültige Skills über MCP veröffentlicht. Starten Sie Codex neu.",
    "A origem foi movida para uma pasta exclusiva e as habilidades válidas foram publicadas pelo MCP. Reinicie o Codex."
  ],
  "ソースフォルダーを移動し、有効なスキルをMCP公開しました。Codexを再起動してください。": [
    "Moved the source folder and published valid skills via MCP. Restart Codex.",
    "已移动来源文件夹，并通过 MCP 公开有效技能。请重启 Codex。",
    "已移動來源資料夾，並透過 MCP 公開有效技能。請重新啟動 Codex。",
    "소스 폴더를 이동하고 유효한 스킬을 MCP에 공개했습니다. Codex를 다시 시작하세요.",
    "Se movió la carpeta de origen y se publicaron las habilidades válidas mediante MCP. Reinicia Codex.",
    "Le dossier source a été déplacé et les compétences valides ont été publiées via MCP. Redémarrez Codex.",
    "Quellordner verschoben und gültige Skills über MCP veröffentlicht. Starten Sie Codex neu.",
    "A pasta de origem foi movida e as habilidades válidas foram publicadas pelo MCP. Reinicie o Codex."
  ],
  "登録後の移動を完了できませんでした: {detail}。現在の登録先・元フォルダー・移動先・公開状態を確認してください。": [
    "Could not complete the move after registration: {detail}. Check the registered location, original folder, destination and publication status.",
    "注册后未能完成移动：{detail}。请检查当前注册位置、原文件夹、目标和公开状态。",
    "註冊後未能完成移動：{detail}。請檢查目前註冊位置、原資料夾、目標與公開狀態。",
    "등록 후 이동을 완료하지 못했습니다: {detail}. 현재 등록 위치, 원본 폴더, 대상 및 공개 상태를 확인하세요.",
    "No se pudo completar el traslado tras registrar: {detail}. Revisa el registro, la carpeta original, el destino y la publicación.",
    "Déplacement après enregistrement impossible : {detail}. Vérifiez l’emplacement enregistré, le dossier d’origine, la destination et la publication.",
    "Verschieben nach Registrierung fehlgeschlagen: {detail}. Prüfen Sie Registrierung, ursprünglichen Ordner, Ziel und Veröffentlichungsstatus.",
    "Não foi possível concluir a mudança após registrar: {detail}. Verifique o registro, a pasta original, o destino e a publicação."
  ],
  "移動を完了できませんでした: {detail}。現在の登録先・元フォルダー・移動先・公開状態を確認してください。": [
    "Could not complete the move: {detail}. Check the registered location, original folder, destination and publication status.",
    "未能完成移动：{detail}。请检查当前注册位置、原文件夹、目标和公开状态。",
    "未能完成移動：{detail}。請檢查目前註冊位置、原資料夾、目標與公開狀態。",
    "이동을 완료하지 못했습니다: {detail}. 현재 등록 위치, 원본 폴더, 대상 및 공개 상태를 확인하세요.",
    "No se pudo completar el traslado: {detail}. Revisa el registro, la carpeta original, el destino y la publicación.",
    "Déplacement impossible : {detail}. Vérifiez l’emplacement enregistré, le dossier d’origine, la destination et la publication.",
    "Verschieben fehlgeschlagen: {detail}. Prüfen Sie Registrierung, ursprünglichen Ordner, Ziel und Veröffentlichungsstatus.",
    "Não foi possível concluir a mudança: {detail}. Verifique o registro, a pasta original, o destino e a publicação."
  ],
  "保存できませんでした: {detail}。下書きは下の編集欄に保持しています。競合時は下書きをコピーしてから「最新版を開く」で最新内容へ統合してください。": [
    "Could not save: {detail}. Your draft remains in the editor below. If there is a conflict, copy your draft, then open the latest version and merge your changes.",
    "无法保存：{detail}。草稿已保留在下方编辑栏。若有冲突，请先复制草稿，再打开最新版本并合并修改。",
    "無法儲存：{detail}。草稿已保留於下方編輯欄。若有衝突，請先複製草稿，再開啟最新版本並合併變更。",
    "저장하지 못했습니다: {detail}. 초안은 아래 편집기에 유지됩니다. 충돌 시 먼저 초안을 복사한 후 최신 버전을 열어 변경 사항을 병합하세요.",
    "No se pudo guardar: {detail}. El borrador permanece en el editor. Si hay conflicto, cópialo, abre la última versión y combina los cambios.",
    "Enregistrement impossible : {detail}. Le brouillon reste dans l’éditeur. En cas de conflit, copiez-le, ouvrez la dernière version et fusionnez vos modifications.",
    "Speichern fehlgeschlagen: {detail}. Ihr Entwurf bleibt im Editor. Kopieren Sie bei Konflikten den Entwurf, öffnen Sie die aktuelle Version und führen Sie die Änderungen zusammen.",
    "Não foi possível salvar: {detail}. O rascunho permanece no editor. Em caso de conflito, copie-o, abra a versão mais recente e mescle suas alterações."
  ],
  "最新内容の取得にも失敗しました: {detail}": [
    "Could not retrieve the latest content either: {detail}",
    "也无法获取最新内容：{detail}",
    "也無法取得最新內容：{detail}",
    "최신 내용도 가져오지 못했습니다: {detail}",
    "Tampoco se pudo obtener el contenido más reciente: {detail}",
    "Impossible de récupérer le contenu le plus récent : {detail}",
    "Auch der aktuelle Inhalt konnte nicht abgerufen werden: {detail}",
    "Também não foi possível obter o conteúdo mais recente: {detail}"
  ]
};

const languages = ["en", "zh-CN", "zh-TW", "ko", "es", "fr", "de", "pt-BR"];
const supported = ["ja", ...languages];
const storageKey = "skilvella.language";
let preference = "auto";
try { const stored = localStorage.getItem(storageKey); if (stored === "auto" || supported.includes(stored)) preference = stored; } catch {}
function resolveLanguage(value) {
  const tag = String(value || "").toLowerCase();
  if (tag === "zh" || tag.startsWith("zh-")) return /(?:^|-)(tw|hk|mo|hant)(?:-|$)/.test(tag) ? "zh-TW" : "zh-CN";
  if (tag === "pt" || tag.startsWith("pt-")) return "pt-BR";
  return supported.find(lang => tag === lang.toLowerCase() || tag.startsWith(lang.toLowerCase() + "-"));
}
function currentLanguage() {
  if (preference !== "auto") return preference;
  for (const language of navigator.languages || [navigator.language]) {
    const resolved = resolveLanguage(language);
    if (resolved) return resolved;
  }
  return "en";
}
let language = currentLanguage();
document.documentElement.lang = language;
const patterns = Object.keys(catalog).filter(key => /\{\w+\}/.test(key)).map(key => {
  const names = [...key.matchAll(/\{(\w+)\}/g)].map(match => match[1]);
  const escaped = key.split(/\{\w+\}/).map(part => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  return { key, names, pattern: new RegExp("^" + escaped.join("([\\s\\S]+?)") + "$") };
});
function t(key, values = {}) {
  let source = key;
  if (!Object.hasOwn(catalog, source)) {
    const match = patterns.map(item => ({ item, match: key.match(item.pattern) })).find(item => item.match);
    if (match) { source = match.item.key; values = Object.fromEntries(match.item.names.map((name, index) => [name, match.match[index + 1]])); }
    else return key;
  }
  const translated = language === "ja" ? source : catalog[source][languages.indexOf(language)];
  return translated.replace(/\{(\w+)\}/g, (token, name) => Object.hasOwn(values, name) ? values[name] : token);
}
const originalText = new WeakMap();
const originalAttributes = new WeakMap();
function translateText(text) {
  const trimmed = text.trim();
  // A server notice may combine an operation result with a scan report.
  const scan = trimmed.indexOf("スキャン完了:");
  if (scan > 0) return t(trimmed.slice(0, scan)) + " " + t(trimmed.slice(scan));
  const latest = trimmed.indexOf("最新内容の取得にも失敗しました:");
  if (latest > 0) return translateText(trimmed.slice(0, latest)) + " " + t(trimmed.slice(latest));
  return t(trimmed);
}
function applyLanguage() {
  language = currentLanguage();
  document.documentElement.lang = language;
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  while (walker.nextNode()) {
    const node = walker.currentNode;
    const marked = node.previousSibling?.nodeType === Node.COMMENT_NODE && node.previousSibling.data === "i18n";
    const notice = node.parentElement?.matches(".notice[role=status], .error-page .notice");
    if (!marked && !notice) continue;
    if (!originalText.has(node)) originalText.set(node, node.textContent);
    const original = originalText.get(node);
    node.textContent = original.replace(original.trim(), translateText(original));
  }
  for (const element of document.querySelectorAll("[placeholder], [data-busy], [data-confirm], [aria-label]")) {
    let saved = originalAttributes.get(element);
    if (!saved) { saved = {}; originalAttributes.set(element, saved); }
    for (const attr of ["placeholder", "data-busy", "data-confirm", "aria-label"]) {
      if (!element.hasAttribute(attr) || (attr === "aria-label" && element.hasAttribute("data-user-label"))) continue;
      if (!Object.hasOwn(saved, attr)) saved[attr] = element.getAttribute(attr);
      element.setAttribute(attr, t(saved[attr]));
    }
  }
  const title = document.querySelector("title[data-translate=true]");
  if (title) {
    if (!originalText.has(title)) originalText.set(title, document.title);
    document.title = t(originalText.get(title));
  }
  document.dispatchEvent(new Event("skilvella:language"));
}
window.skilvellaI18n = { t, applyLanguage, resolveLanguage };
document.addEventListener("DOMContentLoaded", () => {
  const selector = document.querySelector("[data-language-selector]");
  if (selector) {
    selector.value = preference;
    selector.disabled = false;
    selector.addEventListener("change", () => {
      preference = selector.value === "auto" || supported.includes(selector.value) ? selector.value : "auto";
      let saved = true;
      try { localStorage.setItem(storageKey, preference); } catch { saved = false; }
      applyLanguage();
      const status = document.querySelector("[data-preference-status]");
      if (status) { status.hidden = saved; status.textContent = t("設定を保存できませんでした。このタブには適用されています。"); }
    });
  }
  applyLanguage();
});
window.addEventListener("languagechange", () => { if (preference === "auto") applyLanguage(); });
window.addEventListener("storage", event => {
  if (event.key !== storageKey && event.key !== null) return;
  preference = event.key === null || !supported.includes(event.newValue) ? "auto" : event.newValue;
  const selector = document.querySelector("[data-language-selector]");
  if (selector) selector.value = preference;
  applyLanguage();
});
})();
