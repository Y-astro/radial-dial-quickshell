pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenu — Transparent overlay window loader with active keyboard focus
// Owns all menu state so host configurations do not need GlobalStates patches.
// ─────────────────────────────────────────────────────────────────────────────
Scope {
    id: root

    property bool internalOpen: false
    property real internalX: 0
    property real internalY: 0
    property var  internalScreen: null
    property var  internalContextWin: ({})
    property bool highPowerMode: false

    readonly property bool isMenuOpen: internalOpen
    readonly property real posX: internalX
    readonly property real posY: internalY
    readonly property var targetScreen: internalScreen ?? Quickshell.screens[0]
    readonly property string ipcScriptPath: {
        const url = Qt.resolvedUrl("hypr_ipc.py").toString()
        return url.startsWith("file://") ? url.substring(7) : url
    }

    Process {
        id: renderModeDetector
        command: ["python3", root.ipcScriptPath, "render_mode"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: root.highPowerMode = text.trim() === "gpu"
        }
    }

    // Always register the shortcut here. The host only needs to dispatch
    // quickshell:radialMenu; no host-side QML state or signal is required.
    GlobalShortcut {
        name: "radialMenu"
        description: "Open radial pie menu at cursor"
        onPressed: {
            if (root.isMenuOpen) {
                root.closeAnimated()
                return
            }
            if (!cursorReaderProc.running) cursorReaderProc.running = true
        }
    }

    // Fetch cursor, active-window, tabs, and clients in one short-lived process.
    Process {
        id: cursorReaderProc
        command: ["python3", root.ipcScriptPath, "context"]
        running: false
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    const data = JSON.parse(text.trim())
                    const pos = data.cursor || { x: 0, y: 0 }
                    let screen = null
                    for (let i = 0; i < Quickshell.screens.length; i++) {
                        const s = Quickshell.screens[i]
                        if (pos.x >= s.x && pos.x < s.x + s.width &&
                            pos.y >= s.y && pos.y < s.y + s.height) {
                            screen = s
                            break
                        }
                    }
                    if (!screen) {
                        const name = Hyprland.focusedMonitor?.name
                        screen = Quickshell.screens.find(s => s.name === name) ?? Quickshell.screens[0]
                    }
                    root.internalContextWin = data.window || {}
                    RadialMenuActions.cachedTabs = data.tabs || []
                    RadialMenuActions.cachedClients = data.clients || []
                    root.internalScreen = screen
                    root.internalX = pos.x - screen.x
                    root.internalY = pos.y - screen.y
                    root.internalOpen = true
                } catch(e) {
                    console.warn("[RadialMenu] Could not read context:", e)
                }
            }
        }
    }

    function closeMenu() {
        root.internalOpen = false
    }

    function closeAnimated() {
        if (menuLoader.item && typeof menuLoader.item.closeAnimated === "function") {
            menuLoader.item.closeAnimated()
        } else {
            root.closeMenu()
        }
    }

    Loader {
        id: menuLoader
        active: root.isMenuOpen

        sourceComponent: PanelWindow {
            id: menuWin

            function closeAnimated() {
                radialContent.closeAnimated()
            }

            screen: root.targetScreen
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0

            WlrLayershell.namespace: "quickshell:radialMenu"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: (radialContent && !radialContent.isClosing) ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            // Click outside the dial closes the menu with outside-to-inside animation
            MouseArea {
                anchors.fill: parent
                enabled: radialContent ? !radialContent.isClosing : true
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                onClicked: {
                    if (radialContent && radialContent.isCustomizerActive) {
                        radialContent.closeCustomizer()
                        return
                    }
                    menuWin.closeAnimated()
                }
            }

            RadialMenuContent {
                id: radialContent
                centerX: root.posX
                centerY: root.posY
                contextWindow: root.internalContextWin
                highPowerMode: root.highPowerMode
                focus: true
                onMenuClosed: root.closeMenu()
            }
        }
    }
}
