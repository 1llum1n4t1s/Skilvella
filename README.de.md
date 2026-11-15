# Skilvella

[日本語](README.ja.md) | [English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | **Deutsch** | [Português (Brasil)](README.pt-BR.md)

Eine lokale App, die große Sammlungen von `SKILL.md`-Dateien im Browser organisiert und über MCP nur die Skills lädt, die Sie benötigen. Egal wie viele Skills Sie haben, es werden nur drei MCP-Tools bereitgestellt.

- Stichwortsuche in Namen, Beschreibungen, Inhalten und Tags, einschließlich japanischer Teilstringsuche.
- Ordner registrieren, Skills erstellen, Inhalte bearbeiten, Tags und Kategorien, Sammelvorgänge und eine Umschaltung der MCP-Veröffentlichung pro Skill.
- Vor jeder Bearbeitung gespeicherter Verlauf mit Wiederherstellung, Konflikterkennung bei externen Änderungen sowie Prüfungen auf doppelte Namen, Syntaxfehler und fehlende Dateien.
- Windows / macOS / Linux. Verwenden Sie den Browser auf demselben PC.

## Erste Schritte

Führen Sie unter Windows [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe) aus und starten Sie „Skilvella“ aus dem Startmenü. Die Installer-Version (Setup) richtet sich bei der Installation so ein, dass sie startet, sobald sich der aktuelle Benutzer anmeldet. Wenn Sie von einer bestehenden eigenständigen Version (ZIP) wechseln, beenden Sie zuerst die alte Version über ihr Symbol im Infobereich der Taskleiste (läuft die alte Version noch, ist der Standardport (7465) nicht verfügbar, und die Installer-Version (Setup) zeigt eine entsprechende Meldung an und startet nicht). Die Autostart-Registrierung der ZIP-Version bei der Anmeldung wird bei der Installation automatisch auf die Installer-Version (Setup) umgestellt, und die Startmenü-Verknüpfung „Skilvella“ verweist danach ebenfalls auf die Installer-Version (Setup). Wenn Sie den Autostart deaktiviert hatten, bleibt er deaktiviert. Beim Deinstallieren wird auch die Autostart-Registrierung entfernt, die auf die Installer-Version (Setup) verweist. Der Ordner der alten Version wird nicht automatisch gelöscht; entfernen Sie ihn von Hand, wenn Sie ihn nicht mehr brauchen. Wenn Sie eine alte Installer-Version (Setup) ohne Auto-Update-Mechanismus (1.1.4 bis 1.1.6) verwenden, installieren Sie einmalig mit dem neuen Setup neu. Der Standard-Datenordner `%LOCALAPPDATA%\Skilvella` wird gemeinsam genutzt und bleibt nach der Deinstallation erhalten. Die Installer-Version aktualisiert sich automatisch ([App-Updates](#app-updates)).

Für die ZIP-Version oder die macOS-Version laden Sie das passende Paket ([Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) oder [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz), bereitgestellt auf skilvella.kagayoi.com) herunter und entpacken es. Die ZIP- und macOS-Versionen bis 1.1.8 suchten Updates auf GitHub, was nicht mehr verwendet wird; wenn Sie eine davon nutzen, laden Sie das neue Paket einmalig manuell herunter.

- Windows x64: Entpacken Sie `Skilvella-windows-x86_64.zip` in den Ordner, den Sie dauerhaft verwenden möchten, und starten Sie dann `skilvella.exe`. Die App läuft im Infobereich der Taskleiste, ohne ein Terminal zu öffnen, und der erste normale Start fügt „Skilvella“ zum Startmenü des aktuellen Benutzers hinzu.
- Mac Apple Silicon: Entpacken Sie `Skilvella-macos-aarch64.tar.gz` und führen Sie `./skilvella` in einem Terminal aus.
- Vor dem Entpacken können Sie die SHA-256-Prüfsumme des Pakets anhand von [`SHA256SUMS.txt`](https://skilvella.kagayoi.com/portable/SHA256SUMS.txt) verifizieren.

Für Linux oder das Bauen aus dem Quellcode siehe die [Entwicklungsanleitung](docs/DEVELOPMENT.md).

Skilvella zeigt sein Symbol unter Windows im Infobereich der Taskleiste und auf dem Mac in der Menüleiste an. Ein Linksklick öffnet die Verwaltung in Ihrem Standardbrowser. Mit einem Rechtsklick (auf dem Mac Control-Klick) wählen Sie „Manager öffnen“, „Nach Updates suchen“, „Dienst neu starten“ oder „Beenden“. Wenn Sie das Symbol unter Windows nicht sehen, prüfen Sie auch den Überlaufbereich der ausgeblendeten Symbole in der Taskleiste.

„Dienst neu starten“ startet den HTTP/MCP-Server und die Dateiüberwachung im selben Prozess und unter derselben URL neu. Gespeicherte Skills und Einstellungen bleiben erhalten. Doppelte Anforderungen während eines Neustarts werden zusammengefasst, und der Neustart wartet, bis laufende HTTP-Vorgänge abgeschlossen sind. MCP-Verbindungen werden getrennt, verbinden Sie sich also von Ihrem Client aus neu. Laden Sie jede geöffnete Verwaltungsseite neu, bevor Sie sie verwenden. „Beenden“ fährt den Server und das Symbol herunter.

Sie können die Verwaltung auch direkt unter [http://127.0.0.1:7465/](http://127.0.0.1:7465/) öffnen. Der MCP-Endpunkt ist `http://127.0.0.1:7465/mcp`. Übergeben Sie in einer Umgebung ohne Display `--no-tray`. Unter Linux wird kein Symbol angezeigt, und die App läuft als HTTP/MCP-Server.

Beim eigenständigen Windows-Layout versucht ein Start ohne Argumente, die App so zu registrieren, dass sie bei der Anmeldung des aktuellen Benutzers startet. Ist sie bereits registriert, ändert das Starten der ausführbaren Datei von einem anderen Ort den registrierten Pfad nicht. Sie können den Autostart in den Windows-„Einstellungen“ → „Apps“ → „Autostart“ ein- oder ausschalten. Um die Registrierung für diesen Start zu überspringen, übergeben Sie `--no-autostart`. Für den Autostart unter Toolport-Verwaltung konfigurieren Sie ihn auf der Toolport-Seite.

Zur Laufzeit werden weder Rust, Node.js, Python noch ein externer Datenbankserver benötigt. Wenn die App aus einem Terminal gestartet wurde, können Sie sie auch mit `Strg+C` in der zugehörigen Konsole beenden.

## Bibliothek verwalten

Beim ersten Start erkennt Skilvella den verwalteten Ordner automatisch. Es prüft in dieser Reihenfolge die vorhandenen `~/.agents/skills`, `$CODEX_HOME/skills` und `~/.codex/skills`, und wenn keiner gefunden wird, erstellt es `<data-folder-name>-skills` neben dem Datenordner. Ein Kandidat, der nur von Skilvella erzeugte Codex-Aufrufeinträge enthält, wird übersprungen, und die Einträge selbst werden nicht als verwaltete Skills gezählt. Das gilt nur, wenn der Datenordner einen Eigentumsnachweis für diese Einträge enthält. Wenn Sie den Datenordner neu anlegen oder mit `--data-dir` auf einen anderen verweisen, werden zuvor erzeugte, noch vorhandene Einträge als gewöhnliche Skills registriert. Waren zuvor Quellen registriert, hat diese Konfiguration Vorrang und wird übernommen. Verhindert ein Problem beim Lesen des Eigentumsnachweises der Einträge oder der Kandidaten, dass die automatische Erkennung abgeschlossen werden kann, bleibt das gespeicherte Ziel der automatischen Registrierung erhalten. Prüfen Sie den Erkennungsstatus in der Ordnerliste links in der Bibliothek.

Die Bibliothek besteht aus einer einzigen Ansicht: links die Liste der verwalteten Ordner, rechts die Liste der Skills. Mit „+ Ordner hinzufügen“ öffnen Sie die Standard-Ordnerauswahl des Betriebssystems; der gewählte Ordner wird unverändert registriert und gescannt. Dateien werden nicht verschoben. Sie müssen keinen Namen eingeben, und der Ordnername wird als Anzeigename verwendet (Sie können ihn später mit „Anzeigenamen ändern“ ändern). In Umgebungen, in denen der Dialog nicht geöffnet werden kann (etwa SSH-Sitzungen), geben Sie unter „Pfad eingeben“ einen absoluten Pfad ein.

Wenn Sie links einen Ordner auswählen, werden sein Speicherort, die Anzahl und der Hinweis zum Standardsuchpfad angezeigt, zusammen mit „Anzeigenamen ändern“ und „Verwaltung beenden“. Beim Standardordner können Sie mit „Referenzierten Ordner ändern“ den referenzierten Ordner wechseln und mit „Durchsuchen…“ einen im Dialog auswählen. Die Einstellung bleibt über Neustarts hinweg bestehen, und „Verwaltung beenden“ löscht keine Dateien. Wenn Sie einen bereits verwalteten Ordner oder einen Ordner wählen, der in einer Eltern-Kind-Beziehung zu einem verwalteten Ordner steht, wird das Hinzufügen abgebrochen und der Name des überschneidenden Ordners angezeigt.

Neue Skills, die durch Hinzufügen, automatische Erkennung oder externe Änderungen gefunden werden, werden mit **MCP-Veröffentlichung eingeschaltet** registriert, sofern sie gelesen werden können. Skills mit Lesefehlern oder ungültigem Frontmatter bleiben unveröffentlicht und werden auch nach einer Korrektur nicht automatisch veröffentlicht. Der Veröffentlichungsstatus vorhandener Skills ändert sich weder beim erneuten Scannen noch beim Start (von Ihnen unveröffentlichte Skills werden nie automatisch veröffentlicht). Unter Windows und macOS wird ein Ordner, dessen Name sich nur in der ASCII-Groß-/Kleinschreibung unterscheidet, als derselbe Skill behandelt, und sein Veröffentlichungsstatus ändert sich nicht. Wenn Sie einen Skill nicht veröffentlichen möchten, wählen Sie nach dem Hinzufügen des Ordners in der Spalte „Status“ der Liste „Nicht veröffentlicht“. Die Auswahl von „Veröffentlicht“ oder „Nicht veröffentlicht“ in der Spalte „Status“ der Liste wird sofort gespeichert. Sie können sie auch auf der Einzelseite oder mit den Sammelvorgängen der Liste umschalten. Beim Wechsel auf „Veröffentlicht“ werden die Dateien erneut geprüft, und Skills, die fehlen, Lesefehler haben oder ungültiges Frontmatter enthalten, werden nicht veröffentlicht; die Auswahl wird zurückgesetzt und der Grund angezeigt. Tags und Kategorien ändern sich nicht.

Die Bibliothek listet von Anfang an die Skills aller Ordner auf und lässt Sie direkt darin suchen. Ein Ordner, der direkt in sich ein `.git`-Verzeichnis oder eine `.git`-Datei enthält, wird als Quelle (Repository) unter dem Ordner verschachtelt angezeigt, alles andere wird unter „Persönliche Skills“ gruppiert. Ist ein Ordner ohne direkt darin liegendes Quell-Repository selbst ein Git-Repository, wird der gesamte Ordner zu einer Quelle. Die normale Liste zeigt die derzeit vorhandenen Skills. Alte Registrierungen, etwa aus der Zeit vor einem Verschieben, bleiben erhalten und lassen sich unter dem Status „Nicht gefunden“ oder „Alle“ sowie unter „Einträge fehlender Skills bereinigen“ links prüfen. In der Liste „Nicht gefunden“ löscht „Fehlende Einträge löschen“ die Registrierungsdaten, Einstellungen und den Verlauf über alle Seiten, die den Suchbedingungen entsprechen. Das lässt sich nicht rückgängig machen. Es ändert keine tatsächlichen Dateien und bricht das Löschen aller Einträge ab, wenn die Quelle nicht erreichbar ist oder ein Pfad existiert. Kategorien lassen sich in den Metadaten jedes Skills bearbeiten.

Skilvella durchsucht die `SKILL.md`-Dateien in registrierten Ordnern **rekursiv ohne Tiefenbegrenzung**. Es scannt auch nach dem Fund eines Skills tiefere Unterverzeichnisse weiter. Eltern- und Kind-Skills werden mit getrennten IDs und Veröffentlichungseinstellungen verwaltet, und die Dateien eines Kindes gehören nicht zum MCP-Veröffentlichungsbereich des Elternteils. Symbolische Links, Windows-Junctions, `.git`, `node_modules`, `target` und `.work` sind ausgeschlossen. Das Abmelden eines Ordners löscht die Originaldateien nicht. Wenn Sie einen Ordner, den Sie aus der Verwaltung genommen hatten, wieder hinzufügen, werden seine früheren Verwaltungsdaten (Veröffentlichungseinstellungen, Tags, Verlauf) wiederhergestellt. Gleichnamige Ordner stehen nicht in Konflikt miteinander.

Das Speichern von Inhaltsänderungen ändert die tatsächliche Datei am registrierten Ort. Die Vorschau deaktiviert HTML, das Laden von Bildern und Link-Aktionen und führt keine Skripte aus. Änderungen in externen Editoren werden automatisch überwacht. Werden sie nicht übernommen, verwenden Sie „Erneut scannen“.

Bei Lesefehlern öffnen Sie die ordnerübergreifende Fehlerliste über „Lesefehler prüfen und beheben“ in der Bibliothek oder über „Lesefehler“ links. Öffnen Sie den Skill-Namen oder Pfad, um die Ursache zu prüfen, und korrigieren und speichern Sie bei Syntaxfehlern den Inhalt. Kann eine Datei nicht gelesen werden, prüfen Sie Speicherort, Berechtigungen und Zeichenkodierung und scannen Sie dann erneut. Der Hinweis verschwindet, sobald der Fehler behoben ist.

<a id="app-updates"></a>

## App-Updates

**Installer-Version (Setup)**: Automatische Updates sind standardmäßig eingeschaltet. Sie prüft beim Start und während des Betriebs alle 24 Stunden, ob eine neue Version vorliegt, und lädt sie bei Verfügbarkeit herunter. Das Update wird beim nächsten Start automatisch angewendet, wobei die App mit denselben Einstellungen neu startet. Innerhalb von 24 Stunden nach der vorherigen Prüfung wird keine Verbindung aufgebaut. Schlägt eine Prüfung oder ein Download fehl, läuft die aktuelle Version weiter. Sie können dies unter „App-Updates“ in den „Einstellungen“ mit „Beim Start automatisch aktualisieren“ ausschalten (die Einstellung wird in `app-update.json` im Datenordner gespeichert, etwa `%LOCALAPPDATA%\Skilvella`, und ändert die Skill-Bibliothek nicht). „Nach Updates suchen“ prüft unabhängig vom 24-Stunden-Intervall, und „Update installieren“ lädt das Update herunter und startet sofort neu. Die Update-Informationen werden von `skilvella.kagayoi.com` abgerufen.

Das Folgende gilt für die eigenständige Version (ZIP) und für den Mac.

Sie können im Update-Bereich unter „Einstellungen“ oder über „Nach Updates suchen“ im Tray-Menü nach dem neuesten stabilen Release auf skilvella.kagayoi.com suchen. „Update installieren“ lädt das Paket für Windows x64 / Mac Apple Silicon und `SHA256SUMS.txt` über HTTPS herunter, verifiziert die SHA-256-Prüfsumme und wendet das Update dann an. Dazu werden das vom Betriebssystem mitgelieferte `curl` und Schreibrechte am Speicherort der ausführbaren Datei benötigt. Das Aktualisieren innerhalb der App wird unter Linux oder auf anderen CPUs nicht unterstützt.

Das Update wartet vor dem Beenden, bis laufende HTTP-Vorgänge abgeschlossen sind, und startet dann mit denselben Startargumenten und demselben Arbeitsordner neu. Verbinden Sie Ihre MCP-Clients neu. Schlägt die Paketprüfung fehl, bleibt die aktuelle Version erhalten. Kann der neue Prozess nach dem Anwenden des Updates nicht gestartet werden, versucht die App, die alte ausführbare Datei oder das Toolport-Referenzziel wiederherzustellen. Die Einstellungsseite, die das Update gestartet hat, lädt sich automatisch neu, sobald die neue Version antwortet. Führen Sie einen Update-Vorgang auf einer alten Einstellungsseite aus, die schon vor dem Neustart der App geöffnet war, werden Sie gebeten, die Seite neu zu laden.

Am selben Ort wie die ausführbare Datei vor dem Update wird ein Ordner `.skilvella-update-*` angelegt, der `update.log` enthält. Im normalen Layout speichert er außerdem `previous-binary` zur Wiederherstellung. Nachdem Sie bestätigt haben, dass nach dem Update alles funktioniert und der Update-Helfer beendet ist, können Sie diesen Ordner löschen.

Unter der Verwaltung durch `Skilvella-Latest.ps1` von Toolport bleibt die bestehende Version erhalten, der Ordner der neuen Version und Verifikationsinformationen werden hinzugefügt, und das Referenzziel des Launchers wird umgeschaltet. Der Update-Helfer läuft, bis die neu gestartete App beendet wird, beenden Sie Skilvella also, bevor Sie den Update-Ordner löschen.

Wird Skilvella von einem externen Prozessmonitor gestartet, müssen Sie es eventuell über dieses Werkzeug neu starten. Kann die Toolport-Startsperre nicht erlangt werden, wird das Update nicht angewendet. Startet die App nicht, führen Sie den Launcher erneut aus. Ein von Toolport verwalteter Speicherort mit Nicht-ASCII-Zeichen, etwa japanischen, unterstützt das Aktualisieren innerhalb der App nicht. Das normale eigenständige Layout ist von dieser Einschränkung nicht betroffen.

<a id="mcp-connection"></a>

## MCP-Verbindung

Registrieren Sie die folgende URL bei einem Client, der Streamable HTTP unterstützt.

```text
http://127.0.0.1:7465/mcp
```

Registrieren Sie sie in Toolport als HTTP-Server und geben Sie dieselbe URL an. Lassen Sie Skilvella laufen.

| Tool | Zweck |
| --- | --- |
| `search_skills` | Veröffentlichte Skills durchsuchen und Zusammenfassungen und IDs abrufen |
| `load_skill` | Den Inhalt von `SKILL.md` und die Liste der Referenzdateien anhand einer ID abrufen |
| `read_skill_file` | UTF-8-Text unter einem Skill zeilenweise abrufen |

Übergeben Sie bei `load_skill` die Fortsetzungsargumente weiter, bis `next` null ist, fügen Sie den Inhalt zusammen und befolgen Sie dann die Anweisungen. Die Liste umfasst normalerweise 100 Einträge pro Seite und insgesamt bis zu 8 KiB an Pfaden, überschreitet aber ein einzelner Pfad 8 KiB, wird dieser Pfad allein zurückgegeben, sodass die Antwort groß wird. Rufen Sie Skills und die dafür benötigten Referenzen einzeln ab. Verwenden Sie bei Referenzen `read_skill_file`, um `next_text_offset` für den benötigten Zeilenbereich bis zum Ende abzurufen, und nutzen Sie `next_start_line` nur, wenn weitere Zeilen benötigt werden. Byte-Seiten desselben Zeilenbereichs werden unverändert aneinandergefügt, und beim Zusammenfügen von Zeilenbereichen werden Zeilenumbrüche eingefügt. Behalten Sie beim Fortsetzen mit `next_text_offset` `start_line` und `max_lines` bei; setzen Sie für den nächsten Zeilenbereich `start_line` auf `next_start_line` und `text_offset` auf 0 zurück. Zeilenumbrüche werden wie bisher normalisiert, und der abschließende Zeilenumbruch wird entfernt. Geben Sie bei Fortsetzungen die zurückgegebene `revision` als `expected_revision` an. Hat sich die Datei zwischenzeitlich geändert, rufen Sie sie erneut von Anfang an ab.

MCP ist schreibgeschützt. Es kann weder den Inhalt unveröffentlichter Skills noch von Skills mit ungültigem Frontmatter oder fehlenden Skills zurückgeben. Es gibt auch keine Funktion zum Ausführen von Referenzdateien. Inhalte sind auf 2 MiB und Referenztext auf 1 MiB begrenzt.

Die MCP-Veröffentlichung allein ändert nichts daran, dass Agenten standardmäßig dieselben Skills entdecken und laden. Um doppeltes Laden zu vermeiden, legen Sie Skills außerhalb der Standardsuchpfade des Agenten ab und registrieren diesen Ort mit „+ Ordner hinzufügen“ in der Bibliothek. Skilvella hat keine Funktion zum Verschieben von Ordnern. Müssen vorhandene Dateien verschoben werden, tun Sie das selbst und registrieren den neuen Ort erneut. Die Konfigurationsdateien von Codex werden nicht bearbeitet. Starten Sie Codex nach einer Änderung der Standardsuchpfade neu. Bereits in eine Unterhaltung geladene Inhalte werden nicht entfernt. Prüfen Sie benutzerdefinierte Suchpfad-Einstellungen und andere Clients separat.

## Automatische Auswahl und Befehlsvorschläge

Registrieren und verifizieren Sie Skilvella zuerst bei einem Client, der die [MCP-Verbindung](#mcp-connection) verwendet. Aktivieren Sie in den „Einstellungen“ unter „Synchronisierung der Aufrufeinträge“ die Synchronisierungsziele (Codex, Claude Code, Grok; Mehrfachauswahl möglich) und speichern Sie, und Skilvella synchronisiert die Aufrufeinträge für jeden Client selbst automatisch. Wenn Sie in einer älteren Version „Codex-Integration“ aktiviert hatten, wird dies mit ausgewähltem Codex übernommen. Das funktioniert unter Windows, macOS und Linux gleich und verwendet weder die Aufgabenplanung noch einen separaten residenten Prozess. Die Synchronisierung läuft beim Start, nach Verwaltungsvorgängen und bei periodischen Prüfungen, und die Einstellung bleibt nach einem Neustart des Dienstes erhalten.

| Synchronisierungsziel | Speicherort des Eintrags | Wie er der Suche zugänglich gemacht wird | Steuerung der automatischen Auswahl |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Echte Dateien (Codex durchsucht verschachtelte Ordner) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | Ein Link nach `~/.claude/skills/<name>` (unter Windows eine Junction, unter macOS und Linux ein symbolischer Link) | `disable-model-invocation` |
| Grok | `~/.grok/skills/skilvella/<name>` | Echte Dateien (Grok durchsucht verschachtelte Ordner) | `disable-model-invocation` |

Claude Code durchsucht nur eine Ebene direkt unter `~/.claude/skills`, daher werden die Einträge außerhalb des Suchpfads abgelegt und für jeden Skill wird ein Link erstellt. Ist in `~/.claude/skills` bereits ein vorhandener Skill oder Link mit demselben Namen vorhanden, wird er nicht überschrieben, und der Name wird als Synchronisierungsfehler angezeigt. Ein Link, den Sie nach der Anleitung einer alten Version von Hand von `~/.claude/skills/<name>` auf `~/.agents/skills/skilvella/<name>` angelegt haben, wird auf den Claude-Code-Eintrag umgelenkt und zur Verwaltung übernommen, aber nur für Namen, die Synchronisierungsziele sind. Links für Namen, die keine Synchronisierungsziele sind, bleiben unangetastet (deaktivieren Sie Codex, verschwindet ihr Referenzziel). Wird die Synchronisierung für Claude Code deaktiviert, werden nur die Links entfernt, die Skilvella erstellt oder übernommen hat. Wenn Sie `~/.claude/skills` selbst als verwalteten Ordner registriert haben, findet keine Synchronisierung statt.

Grok liest auch die Suchpfade von Codex (`~/.agents/skills`) und Claude Code (`~/.claude/skills`), und bei Skills mit demselben Namen gilt die Priorität `~/.grok/skills`, `~/.agents/skills`, `~/.claude/skills`. Codex-Einträge stoppen die automatische Auswahl über `agents/openai.yaml`, sodass in Grok sogar Skills mit „Nur Befehlsvorschläge“ automatisch ausgewählt werden. Wenn Sie Grok verwenden, wählen Sie Grok als Synchronisierungsziel.

Wählen Sie den Modus auf der Detailseite jedes Skills oder mit „Verwendung“ in der Liste. In der Liste sehen Sie den aktuellen Modus und können ihn direkt umschalten. MCP gibt den Inhalt unveröffentlichter Skills nicht zurück und entfernt nach erfolgreicher Synchronisierung auch deren Vorschlagseinträge.

Wenn Sie während eines laufenden Speichervorgangs in der Liste die Zurück-Schaltfläche des Browsers drücken oder neu laden, prüft Skilvella das Speicherergebnis, bevor es die neuesten Einstellungen anzeigt. Eine Anforderung, die wegen einer unterbrochenen Verbindung nicht angenommen wurde, wird abgebrochen, und das Prüfergebnis wird auf dem Bildschirm angezeigt.

| Modus | Für die automatische Auswahl übergebener Name und Beschreibung | Befehlsvorschläge |
| --- | --- | --- |
| Nur Suche (Standard) | Nicht übergeben | Nicht angezeigt |
| Nur Befehlsvorschläge | Nicht übergeben | Angezeigt |
| Automatische Auswahl + Befehlsvorschläge | Übergeben | Angezeigt |

Für Skills mit Befehlsvorschlägen wird der Aufrufbefehl für jedes Synchronisierungsziel (`$name` für Codex, `/name` für Claude Code und Grok) in der Liste unter dem Skill-Namen und auf der Detailseite unter „Verwendung“ angezeigt. Enthält der Name etwas anderes als Kleinbuchstaben, Ziffern und `-`, wird er zu `skill-<first 8 characters of the id>`, und existiert ein veröffentlichter Skill mit demselben Namen, wird er zu `<name>-<first 8 characters of the id>`, verwenden Sie also den angezeigten Befehl. Ist kein Befehl vorhanden, nennt die Detailseite den Grund (nur Suche, kein Synchronisierungsziel ausgewählt, unveröffentlicht oder Lesefehler). Der Befehl für einen Skill, den Sie in der Liste von unveröffentlicht auf veröffentlicht umschalten, erscheint nach einem Neuladen.

Markieren Sie in der Liste Skills, wählen Sie unter den Sammelvorgängen „Verwendung“ und klicken Sie auf „Auf Auswahl anwenden“, um den Modus mehrerer Skills auf einmal zu ändern. „Nicht ändern“ behält den aktuellen Modus bei. Sie können gleichzeitig den Veröffentlichungsstatus und die Tags ändern.

Wenn Sie unter „Alle Skills“ „Sammeleinstellungen nach Quelle“ öffnen, können Sie mehrere Quellen auswählen und mit „Auf ausgewählte Quellen anwenden“ Veröffentlichungsstatus und Verwendung gemeinsam ändern. Der Umfang sind die Skills auf allen Seiten, die zur Laufzeit registriert sind, und er wird nicht auf später hinzugefügte Skills übertragen. Die Anzahl der Vorabprüfung beruht auf dem Scan-Index und schließt fehlende und fehlerhafte Einträge aus. Das Ergebnis zeigt die Anzahl der aktualisierten und der ausgeschlossenen Einträge. Bei der Veröffentlichung werden die Dateien erneut geprüft, und können sie wegen fehlender, unlesbarer, ungültiger oder ähnlicher Dateien nicht erneut validiert werden, wird das Speichern der Einstellungen für alle Einträge abgebrochen. Die Einstellungen werden in einer einzigen Transaktion gespeichert, und die ursprünglichen Skills werden nicht geändert.

Codex hat keine separate Einstellung für „automatische Auswahl aktivieren, aber in den Vorschlägen ausblenden“, daher bietet Skilvella die drei entsprechenden Modi an. Auch bei automatischer Auswahl hält es nicht ständig eine vollständige Kopie des Originalinhalts vor; der Inhalt wird bei Bedarf über MCP abgerufen. Nicht in den Vorschlägen angezeigte Skills lassen sich mit `$skilvella-search` suchen, das nur für den ausdrücklichen Aufruf gedacht ist. Sie können auch direkt `search_skills` von MCP verwenden.

Der Verwendungsmodus gilt für alle Synchronisierungsziele gleich. Die Speicherorte der Einträge entsprechen der Tabelle oben. Wenn Sie die Veröffentlichung eines Skills aufheben oder seinen Modus ändern, werden nicht mehr benötigte Artefakte aus den Suchpfaden verschoben (`skilvella-staging` / `skilvella-retired` direkt unter `~/.agents`, `~/.claude` und `~/.grok`). Werden unbekannte Dateien oder manuelle Änderungen erkannt, werden sie nicht überschrieben, und auf der Einstellungsseite wird ein Synchronisierungsfehler angezeigt. Bleiben wegen unzureichenden Speicherplatzes oder eines erzwungenen Beendens unvollständige temporäre Dateien zurück, stoppt der Vorgang und schützt dabei den vorhandenen intakten Index. Prüfen Sie den Inhalt des angezeigten Pfads und versuchen Sie es dann erneut. Das Deaktivieren der Integration entfernt die ihr gehörenden Einträge aus den Suchpfaden. Solange die Synchronisierung angehalten ist, können frühere Vorschläge und Einstellungen zur automatischen Auswahl bestehen bleiben. Originaldateien, Veröffentlichungsstatus und Bearbeitungsverlauf werden nicht geändert. Aktualisiert sich die Vorschlagsanzeige von Codex nicht, öffnen Sie das Vorschlagsmenü erneut oder starten Sie Codex neu. Claude Code übernimmt Änderungen an `~/.claude/skills` auch in einer laufenden Sitzung, aber wenn Sie `~/.claude/skills` selbst erst nach dem Sitzungsstart angelegt haben, führen Sie `/reload-skills` aus. In eine bestehende Unterhaltung bereits geladene Inhalte lassen sich nicht entfernen.

<a id="display-settings"></a>

## Anzeigeeinstellungen

Wählen Sie in der Bibliothek links einen Ordner aus und öffnen Sie „Anzeigenamen ändern“, um den auf dem Bildschirm angezeigten Ordnernamen zu ändern. Die Eingabe darf einschließlich führender und nachgestellter Leerzeichen höchstens 100 Zeichen lang sein, und Steuerzeichen wie Zeilenumbrüche sind nicht erlaubt. Führende und nachgestellte Leerzeichen werden beim Speichern entfernt, und ein leerer Wert stellt die ursprüngliche Kennung wieder her. Der Anzeigename wird in der App gespeichert, und der tatsächliche Ordnername, die Skill-IDs, Veröffentlichungseinstellungen und der Bearbeitungsverlauf ändern sich nicht. Doppelte Anzeigenamen sind zulässig, aber ein von Ihnen geänderter Name wird zusammen mit seiner ursprünglichen Kennung angezeigt.

In den „Einstellungen“ der Verwaltung können Sie die Sprache als automatisch oder als eine von neun Sprachen wählen (Japanisch, Englisch, Vereinfachtes Chinesisch, Traditionelles Chinesisch, Koreanisch, Spanisch, Französisch, Deutsch, Brasilianisches Portugiesisch). Die Standardeinstellung „Automatisch“ verwendet die Spracheinstellung des Browsers. Die Sprache wird beim Auswählen gespeichert, und ist JavaScript deaktiviert, klicken Sie auf „Speichern“. Die Auswahl wird pro Verbindungsport im selben Browser gespeichert, und andere bereits geöffnete Tabs übernehmen sie beim nächsten Laden einer Seite. Eine Spracheinstellung aus einer älteren Version wird übernommen, wenn Sie die Verwaltung unter derselben Adresse im selben Browser öffnen. Die Übernahme erfordert JavaScript. Skill-Namen, Beschreibungen, Inhalte, Pfade und technische Fehlerdetails behalten ihren Originaltext. Die Menüs im Infobereich der Taskleiste und in der Menüleiste, Befehlszeilenausgaben wie `--help`, Startmeldungen und Fehler sowie die Meldungsfenster der Installer-Version (Setup) werden ebenfalls in denselben neun Sprachen angezeigt. Diese Sprachen werden in der Reihenfolge der Umgebungsvariablen `SKILVELLA_LANGUAGE` (zum Beispiel `ja`, `pt-BR`) → `LC_ALL` → `LC_MESSAGES` → `LANG` → der Anzeigesprache des Betriebssystems bestimmt, und kann sie nicht ermittelt werden, wird Englisch verwendet. Die Namen und Beschreibungen der MCP-Tools sind auf Englisch.

Sie können das Design aus Automatisch, Hell und Dunkel wählen, wenn JavaScript aktiviert ist. Die Auswahl wird im Browser gespeichert, und Automatisch folgt der Farbschema-Einstellung des Browsers.

## Daten und Backups

| Betriebssystem | Standard-Datenort |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Skilvella` |
| macOS | `~/Library/Application Support/Skilvella` |
| Linux | `$XDG_DATA_HOME/skilvella` oder `~/.local/share/skilvella` |

Die maßgebliche Quelle für `SKILL.md` und Referenzdateien sind die registrierten Ordner. Zusätzlich zum Suchindex speichert die `library.sqlite3` des Datenordners auch Tags, Kategorien, Veröffentlichungseinstellungen, registrierte Speicherorte, Anzeigenamen von Ordnern und den Bearbeitungsverlauf. **Wenn Sie die gesamte DB löschen, gehen diese Verwaltungsdaten verloren.**

Beenden Sie für ein Backup zuerst Skilvella und kopieren Sie dann sowohl den Datenordner als auch die registrierten Ordner. Die DB allein kann die aktuellen Skill-Dateien nicht wiederherstellen. Auf einem anderen PC unterscheiden sich die absoluten Pfade der registrierten Quellen, daher wird das unveränderte Teilen der DB zur gleichzeitigen Nutzung nicht unterstützt.

## Optionen und Fehlerbehebung

```sh
skilvella --data-dir /absolute/path/to/data --port 7465 --no-watch
skilvella --help
```

- Wenn der Port belegt ist, beenden Sie den vorhandenen Prozess oder ändern Sie ihn mit `--port`. Verwaltung und MCP verwenden denselben Port.
- Wenn die Verwaltung nach einem Update von einer alten Windows-Version nicht zurückkehrt, starten Sie die aktualisierte `skilvella.exe` einmal im normalen Layout oder unter Toolport-Verwaltung den Launcher einmal.
- Kann ein unterbrochenes Verschieben eines Ordners aus einer alten Version beim Start nicht wiederhergestellt werden, wird der Start abgebrochen. Die ursprünglichen Daten und der Migrationseintrag bleiben erhalten.
- Der Datenort und die Registrierungsorte der Skills dürfen sich nicht überschneiden. Geben Sie getrennte Ordner auf einem lokalen Datenträger an.
- Auch Bearbeitungen in einem anderen Tab werden beim Speichern als Konflikte erkannt. Kopieren Sie den aufbewahrten Entwurf, laden Sie neu und führen Sie ihn mit dem neuesten Inhalt zusammen.
- Die Suche verwendet durch Leerzeichen getrennte UND-Verknüpfung und Teilstringabgleich. Die bedeutungsbasierte Suche nach englischen Skills über eine japanische Anfrage, Embedding-Modelle und Cloud-Synchronisierung sind nicht enthalten.
- Auch wenn der Quellordner offline geht, bleiben die Registrierungsdaten und der Verlauf erhalten. Scannen Sie nach der erneuten Verbindung erneut.

Verbindungen sind auf `127.0.0.1` beschränkt. Die App ist für den Einsatz dort gedacht, wo Sie den Benutzern und Prozessen auf demselben PC vertrauen.

Die Lizenz ist [MIT](LICENSE).

Wenn Sie zur Entwicklung beitragen möchten, siehe die [Entwicklungsanleitung](docs/DEVELOPMENT.md), das [Design](DESIGN.md) und die [Richtlinien für Mitwirkende](AGENTS.md).
