pragma ComponentBehavior: Bound
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Io

// ─────────────────────────────────────────────────────────────────────────────
// RadialMenu — Transparent overlay window loader with active keyboard focus
// Compatible with both end4-pC and standalone Quickshell configurations
// ─────────────────────────────────────────────────────────────────────────────
Scope {
    id: root

    // Standalone fallback properties when GlobalStates is not present
    readonly property bool hasGlobalStates: typeof GlobalStates !== "undefined" && GlobalStates !== null
    property bool internalOpen: false
    property real internalX: 0
    property real internalY: 0
    property var  internalScreen: null
    property var  internalContextWin: ({})
    property var  internalTabs: []

    readonly property bool isMenuOpen: hasGlobalStates ? GlobalStates.radialMenuOpen : internalOpen
    readonly property real posX: hasGlobalStates ? GlobalStates.radialMenuX : internalX
    readonly property real posY: hasGlobalStates ? GlobalStates.radialMenuY : internalY
    readonly property var  targetScreen: hasGlobalStates ? (GlobalStates.radialMenuScreen ?? Quickshell.screens[0]) : (internalScreen ?? Quickshell.screens[0])

    // Built-in GlobalShortcut for standalone Quickshell environments
    GlobalShortcut {
        name: "radialMenu"
        description: "Open radial pie menu at cursor"
        onPressed: {
            if (root.isMenuOpen) {
                if (root.hasGlobalStates) {
                    GlobalStates.radialMenuCloseRequested()
                } else {
                    root.internalOpen = false
                }
                return
            }
            if (root.hasGlobalStates) {
                GlobalStates.radialMenuOpen = true
            } else {
                cursorReaderProc.running = true
            }
        }
    }

    // Standalone cursor & active window reader
    Process {
        id: cursorReaderProc
        command: ["bash", "-c", "WIN=$(hyprctl activewindow -j 2>/dev/null); [[ -z \"$WIN\" || \"$WIN\" == \"Invalid window\"* ]] && WIN=\"{}\"; TABS=$(python3 ~/.config/quickshell/modules/ii/radialMenu/get_browser_tabs.py 2>/dev/null || python3 ~/.config/quickshell/end4-pC/modules/ii/radialMenu/get_browser_tabs.py 2>/dev/null || echo \"[]\"); echo \"{\\\"cursor\\\": $(hyprctl cursorpos -j 2>/dev/null || echo '{\\\"x\\\":0,\\\"y\\\":0}'), \\\"window\\\": $WIN, \\\"tabs\\\": $TABS}\""]
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
                    root.internalTabs = data.tabs || []
                    root.internalScreen = screen
                    root.internalX = pos.x - screen.x
                    root.internalY = pos.y - screen.y
                    root.internalOpen = true
                } catch(e) {
                    root.internalOpen = true
                }
            }
        }
    }

    Loader {
        id: menuLoader
        active: root.isMenuOpen

        sourceComponent: PanelWindow {
            id: menuWin

            screen: root.targetScreen
            color: "transparent"
            exclusionMode: ExclusionMode.Ignore
            exclusiveZone: 0

            WlrLayershell.namespace: "quickshell:radialMenu"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            Connections {
                target: root.hasGlobalStates ? GlobalStates : null
                function onRadialMenuCloseRequested() {
                    radialContent.closeAnimated()
                }
            }

            // Click outside the dial closes the menu with outside-to-inside animation
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                onClicked: {
                    if (root.hasGlobalStates) {
                        radialContent.closeAnimated()
                    } else {
                        radialContent.closeAnimated(() => { root.internalOpen = false })
                    }
                }
            }

            RadialMenuContent {
                id: radialContent
                centerX: root.posX
                centerY: root.posY
                contextWindow: root.hasGlobalStates ? GlobalStates.radialMenuContextWindow : root.internalContextWin
                focus: true
            }
        }
    }
}
