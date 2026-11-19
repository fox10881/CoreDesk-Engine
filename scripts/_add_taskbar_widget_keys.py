"""Adds the taskbar resource widget strings to every locale file."""
import json
import os

ROOT = os.path.join(os.path.dirname(__file__), "..", "locales")

# key -> per-locale translation (zh source strings are the keys; zh files carry none).
T = {
    "任务栏资源小组件": {
        "en": "Taskbar resource widget",
        "ar": "أداة موارد شريط المهام",
        "de": "Taskleisten-Ressourcen-Widget",
        "es": "Widget de recursos de la barra de tareas",
        "fr": "Widget de ressources de la barre des tâches",
        "it": "Widget risorse della barra delle applicazioni",
        "ja": "タスクバー リソース ウィジェット",
        "ko": "작업 표시줄 리소스 위젯",
        "pt-BR": "Widget de recursos da barra de tarefas",
        "ru": "Виджет ресурсов панели задач",
        "zh-TW": "工作列資源小工具",
    },
    "任务栏左侧的原生 CPU / RAM / GPU / 网络监视器 — 零 WebView2 开销；可横向拖动，右键菜单锁定位置": {
        "en": "Native CPU / RAM / GPU / network monitor on the taskbar's left side — zero WebView2 overhead; drag it horizontally and lock its position from the right-click menu",
        "ar": "مراقب أصلي للمعالج والذاكرة والمعالج الرسومي والشبكة على يسار شريط المهام — دون أي عبء من WebView2؛ يُسحب أفقياً ويُقفل موضعه من قائمة الزر الأيمن",
        "de": "Nativer CPU-/RAM-/GPU-/Netzwerk-Monitor links auf der Taskleiste — ohne WebView2-Overhead; horizontal ziehen, Position per Rechtsklickmenü sperren",
        "es": "Monitor nativo de CPU / RAM / GPU / red a la izquierda de la barra de tareas — sin sobrecarga de WebView2; se arrastra en horizontal y la posición se bloquea desde el menú contextual",
        "fr": "Moniteur natif CPU / RAM / GPU / réseau à gauche de la barre des tâches — aucun coût WebView2 ; glisser horizontalement, verrouiller la position via le menu contextuel",
        "it": "Monitor nativo di CPU / RAM / GPU / rete a sinistra della barra delle applicazioni — zero overhead di WebView2; trascinabile orizzontalmente, posizione bloccabile dal menu contestuale",
        "ja": "タスクバー左側のネイティブ CPU / RAM / GPU / ネットワーク モニター — WebView2 のオーバーヘッドなし。横にドラッグして、右クリック メニューで位置をロックできます",
        "ko": "작업 표시줄 왼쪽의 네이티브 CPU / RAM / GPU / 네트워크 모니터 — WebView2 오버헤드 제로; 가로로 드래그하고 우클릭 메뉴로 위치를 잠글 수 있습니다",
        "pt-BR": "Monitor nativo de CPU / RAM / GPU / rede à esquerda da barra de tarefas — zero sobrecarga de WebView2; arraste na horizontal e trave a posição pelo menu de contexto",
        "ru": "Нативный монитор ЦП / ОЗУ / ГП / сети слева на панели задач — без накладных расходов WebView2; перетаскивается по горизонтали, позиция фиксируется через контекстное меню",
        "zh-TW": "工作列左側的原生 CPU / RAM / GPU / 網路監視器 — 零 WebView2 開銷；可橫向拖曳，右鍵選單鎖定位置",
    },
    "锁定位置": {
        "en": "Lock position",
        "ar": "قفل الموضع",
        "de": "Position sperren",
        "es": "Bloquear posición",
        "fr": "Verrouiller la position",
        "it": "Blocca posizione",
        "ja": "位置をロック",
        "ko": "위치 잠금",
        "pt-BR": "Travar posição",
        "ru": "Заблокировать позицию",
        "zh-TW": "鎖定位置",
    },
    "防止误拖动 — 右键菜单中的「锁定位置」与此同步": {
        "en": "Prevents accidental drags — synced with the 'Lock position' item in the right-click menu",
        "ar": "يمنع السحب العرضي — متزامن مع خيار «قفل الموضع» في قائمة الزر الأيمن",
        "de": "Verhindert versehentliches Ziehen — synchronisiert mit „Position sperren“ im Rechtsklickmenü",
        "es": "Evita arrastres accidentales — sincronizado con «Bloquear posición» del menú contextual",
        "fr": "Empêche les glissés accidentels — synchronisé avec « Verrouiller la position » du menu contextuel",
        "it": "Impedisce trascinamenti accidentali — sincronizzato con «Blocca posizione» del menu contestuale",
        "ja": "誤ドラッグを防止 — 右クリック メニューの「位置をロック」と同期します",
        "ko": "실수로 드래그하는 것을 방지 — 우클릭 메뉴의 '위치 잠금'과 동기화됩니다",
        "pt-BR": "Impede arrastos acidentais — sincronizado com «Travar posição» do menu de contexto",
        "ru": "Защищает от случайного перетаскивания — синхронизировано с пунктом «Заблокировать позицию» в контекстном меню",
        "zh-TW": "防止誤拖曳 — 右鍵選單中的「鎖定位置」與此同步",
    },
    "系统资源": {
        "en": "System resources",
        "ar": "موارد النظام",
        "de": "Systemressourcen",
        "es": "Recursos del sistema",
        "fr": "Ressources système",
        "it": "Risorse di sistema",
        "ja": "システム リソース",
        "ko": "시스템 리소스",
        "pt-BR": "Recursos do sistema",
        "ru": "Ресурсы системы",
        "zh-TW": "系統資源",
    },
    "完整监控": {
        "en": "Full monitor",
        "ar": "مراقبة كاملة",
        "de": "Vollmonitor",
        "es": "Monitor completo",
        "fr": "Moniteur complet",
        "it": "Monitor completo",
        "ja": "フル モニター",
        "ko": "전체 모니터",
        "pt-BR": "Monitor completo",
        "ru": "Полный монитор",
        "zh-TW": "完整監控",
    },
    "网络速度": {
        "en": "Network speed",
        "ar": "سرعة الشبكة",
        "de": "Netzwerkgeschwindigkeit",
        "es": "Velocidad de red",
        "fr": "Vitesse réseau",
        "it": "Velocità di rete",
        "ja": "ネットワーク速度",
        "ko": "네트워크 속도",
        "pt-BR": "Velocidade de rede",
        "ru": "Скорость сети",
        "zh-TW": "網路速度",
    },
}

for name in sorted(os.listdir(ROOT)):
    if not name.endswith(".json"):
        continue
    locale = name[:-5]
    path = os.path.join(ROOT, name)
    with open(path, encoding="utf-8") as fh:
        data = json.load(fh)
    for k, tr in T.items():
        if k not in data:
            data[k] = tr.get(locale, tr["en"])
    with open(path, "w", encoding="utf-8", newline="\r\n") as fh:
        json.dump(data, fh, ensure_ascii=False, indent=4)
        fh.write("\n")
    print(f"{name} ok")
